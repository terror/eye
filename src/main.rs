use {
  axum::{extract::State, routing::get, Json, Router},
  clap::Parser,
  serde::Serialize,
  std::{
    fs,
    net::SocketAddr,
    path::{Path, PathBuf},
    process,
    sync::Arc,
  },
  syn::{__private::ToTokens, parse_file, Fields, Item, ItemStruct},
  tokio::net::TcpListener,
  tower_http::cors::CorsLayer,
  tracing::{error, info},
  tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt},
  walkdir::WalkDir,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Graph {
  root: NodeId,
  nodes: Vec<Node>,
}

type NodeId = usize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Node {
  id: NodeId,
  name: String,
  kind: NodeKind,
  children: Vec<NodeId>,
  documentation: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
enum NodeKind {
  Module {
    path: PathBuf,
  },
  Struct {
    fields: Vec<Field>,
  },
  Enum {
    variants: Vec<String>,
  },
  Function {
    arguments: Vec<Field>,
    return_type: Option<String>,
  },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Field {
  name: String,
  #[serde(rename = "typeName")]
  type_name: String,
}

fn analyze(crate_path: &Path) -> Result<Graph> {
  let mut graph = Graph {
    root: 0,
    nodes: Vec::new(),
  };

  let root_node = Node {
    id: 0,
    name: crate_path
      .file_name()
      .unwrap()
      .to_string_lossy()
      .into_owned(),
    kind: NodeKind::Module {
      path: crate_path.to_path_buf(),
    },
    children: Vec::new(),
    documentation: String::new(),
  };

  graph.nodes.push(root_node);

  process_crate(&mut graph, crate_path, 0)?;

  Ok(graph)
}

fn process_crate(
  graph: &mut Graph,
  crate_path: &Path,
  parent_id: NodeId,
) -> Result {
  let entries = WalkDir::new(crate_path).into_iter().filter_map(Result::ok);

  for entry in entries {
    if entry.file_type().is_file()
      && entry.path().extension().map_or(false, |ext| ext == "rs")
    {
      let file_path = entry.path();

      let file_content = fs::read_to_string(file_path)?;

      let syntax = parse_file(&file_content)?;

      let module_name = file_path
        .strip_prefix(crate_path)?
        .to_string_lossy()
        .into_owned();

      let module_id = graph.nodes.len();

      let module_node = Node {
        id: module_id,
        name: module_name,
        kind: NodeKind::Module {
          path: file_path.to_path_buf(),
        },
        children: Vec::new(),
        documentation: String::new(),
      };

      graph.nodes.push(module_node);

      graph.nodes[parent_id].children.push(module_id);

      process_items(graph, &syntax.items, file_path, module_id)?;
    }
  }

  Ok(())
}

fn process_items(
  graph: &mut Graph,
  items: &[Item],
  file_path: &Path,
  parent_id: NodeId,
) -> Result {
  for item in items {
    match item {
      Item::Mod(m) => {
        if let Some((_, items)) = &m.content {
          let module_id = graph.nodes.len();
          let module_node = Node {
            id: module_id,
            name: m.ident.to_string(),
            kind: NodeKind::Module {
              path: file_path.to_path_buf(),
            },
            children: Vec::new(),
            documentation: String::new(),
          };
          graph.nodes.push(module_node);
          graph.nodes[parent_id].children.push(module_id);

          process_items(graph, items, file_path, module_id)?;
        }
      }
      Item::Struct(s) => {
        let struct_id = graph.nodes.len();
        let struct_node = Node {
          id: struct_id,
          name: s.ident.to_string(),
          kind: NodeKind::Struct {
            fields: process_struct_fields(s),
          },
          children: Vec::new(),
          documentation: String::new(),
        };
        graph.nodes.push(struct_node);
        graph.nodes[parent_id].children.push(struct_id);
      }
      Item::Enum(e) => {
        let enum_id = graph.nodes.len();
        let enum_node = Node {
          id: enum_id,
          name: e.ident.to_string(),
          kind: NodeKind::Enum {
            variants: e.variants.iter().map(|v| v.ident.to_string()).collect(),
          },
          children: Vec::new(),
          documentation: String::new(),
        };
        graph.nodes.push(enum_node);
        graph.nodes[parent_id].children.push(enum_id);
      }
      Item::Fn(f) => {
        let fn_id = graph.nodes.len();
        let fn_node = Node {
          id: fn_id,
          name: f.sig.ident.to_string(),
          kind: NodeKind::Function {
            arguments: f
              .sig
              .inputs
              .iter()
              .filter_map(|arg| {
                if let syn::FnArg::Typed(pat_type) = arg {
                  Some(Field {
                    name: pat_type.pat.to_token_stream().to_string(),
                    type_name: pat_type.ty.to_token_stream().to_string(),
                  })
                } else {
                  None
                }
              })
              .collect(),
            return_type: match &f.sig.output {
              syn::ReturnType::Default => None,
              syn::ReturnType::Type(_, ty) => {
                Some(ty.to_token_stream().to_string())
              }
            },
          },
          children: Vec::new(),
          documentation: String::new(),
        };
        graph.nodes.push(fn_node);
        graph.nodes[parent_id].children.push(fn_id);
      }
      _ => {}
    }
  }

  Ok(())
}

fn process_struct_fields(item_struct: &ItemStruct) -> Vec<Field> {
  if let Fields::Named(named_fields) = &item_struct.fields {
    named_fields
      .named
      .iter()
      .map(|field| Field {
        name: field.ident.as_ref().unwrap().to_string(),
        type_name: field.ty.to_token_stream().to_string(),
      })
      .collect()
  } else {
    Vec::new()
  }
}

#[derive(Debug, Parser)]
struct Options {
  #[clap(long, short)]
  crate_path: PathBuf,
}

#[derive(Debug, Parser)]
struct Arguments {
  #[clap(flatten)]
  options: Options,
  #[clap(subcommand)]
  subcommand: Subcommand,
}

impl Arguments {
  async fn run(self) -> Result {
    self.subcommand.run(self.options).await
  }
}

#[derive(Debug, Parser)]
enum Subcommand {
  Serve(Server),
}

impl Subcommand {
  async fn run(self, options: Options) -> Result {
    match self {
      Subcommand::Serve(server) => server.run(options).await,
    }
  }
}

#[derive(Debug, Parser)]
struct Server {
  #[clap(short, long, default_value = "8000")]
  port: u16,
}

impl Server {
  async fn run(self, options: Options) -> Result {
    let addr = SocketAddr::from(([0, 0, 0, 0], self.port));

    info!("Listening on port: {}", addr.port());

    let state = Arc::new(options);

    let router = Router::new()
      .route("/api/graph", get(Self::graph))
      .with_state(state)
      .layer(CorsLayer::permissive());

    let listener = TcpListener::bind(addr).await?;

    axum::serve(listener, router.into_make_service()).await?;

    Ok(())
  }

  async fn graph(State(options): State<Arc<Options>>) -> Json<Graph> {
    match analyze(&options.crate_path) {
      Ok(graph) => Json(graph),
      Err(e) => {
        error!("Error analyzing crate: {:?}", e);

        Json(Graph {
          root: 0,
          nodes: vec![],
        })
      }
    }
  }
}

type Result<T = (), E = anyhow::Error> = std::result::Result<T, E>;

#[tokio::main]
async fn main() {
  tracing_subscriber::registry()
    .with(
      tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "info".into()),
    )
    .with(tracing_subscriber::fmt::layer())
    .init();

  if let Err(error) = Arguments::parse().run().await {
    eprintln!("{error}");
    process::exit(1);
  }
}
