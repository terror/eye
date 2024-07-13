use {
  axum::{extract::State, routing::get, Json, Router},
  cargo_metadata::{MetadataCommand, Package},
  clap::Parser,
  serde::Serialize,
  std::{
    collections::HashSet,
    fs,
    mem::take,
    net::SocketAddr,
    path::{Path, PathBuf},
    process,
    sync::Arc,
  },
  syn::{
    __private::ToTokens, parse_file, visit::Visit, Fields, FnArg, Item,
    ItemStruct, ReturnType,
  },
  tokio::net::TcpListener,
  tower_http::cors::CorsLayer,
  tracing::{error, info},
  tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt},
  walkdir::WalkDir,
};

#[derive(Default, Debug, Serialize)]
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
  source_code: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "content")]
enum NodeKind {
  Workspace {
    path: PathBuf,
  },
  Package {
    path: PathBuf,
  },
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
  Const {
    ty: String,
    value: String,
  },
  Macro {
    macro_rules: bool,
  },
  Static {
    ty: String,
    mutability: bool,
  },
  Trait {
    is_auto: bool,
    is_unsafe: bool,
  },
  TraitAlias {
    generics: String,
  },
  Type {
    generics: String,
  },
  Unknown,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Field {
  name: String,
  #[serde(rename = "typeName")]
  type_name: String,
}

struct Analyzer {
  graph: Graph,
}

impl Analyzer {
  fn new() -> Self {
    Self {
      graph: Graph {
        root: 0,
        nodes: Vec::new(),
      },
    }
  }

  fn analyze(&mut self, crate_path: &Path) -> Result<Graph> {
    let metadata = MetadataCommand::new()
      .manifest_path(crate_path.join("Cargo.toml"))
      .no_deps()
      .exec()?;

    let workspace_members = metadata
      .workspace_members
      .into_iter()
      .collect::<HashSet<_>>();

    let is_proper_workspace = workspace_members.len() > 1;

    if is_proper_workspace {
      self.graph.nodes.push(Node {
        id: 0,
        name: crate_path
          .file_name()
          .unwrap()
          .to_string_lossy()
          .into_owned(),
        kind: NodeKind::Workspace {
          path: crate_path.to_path_buf(),
        },
        children: Vec::new(),
        documentation: String::new(),
        source_code: String::new(),
      });
    }

    for package in metadata.packages {
      if workspace_members.contains(&package.id) {
        self.handle_package(&package, 0, is_proper_workspace)?;
      }
    }

    Ok(take(&mut self.graph))
  }

  fn handle_package(
    &mut self,
    package: &Package,
    parent_id: NodeId,
    is_workspace: bool,
  ) -> Result {
    let package_id = self.graph.nodes.len();

    let package_node = Node {
      id: package_id,
      name: package.name.clone(),
      kind: NodeKind::Package {
        path: package.manifest_path.parent().unwrap().to_path_buf().into(),
      },
      children: Vec::new(),
      documentation: package.description.clone().unwrap_or_default(),
      source_code: String::new(),
    };

    self.graph.nodes.push(package_node);

    if is_workspace {
      self.graph.nodes[parent_id].children.push(package_id);
    }

    let src_path = package.manifest_path.parent().unwrap().join("src");

    let entries = WalkDir::new(&src_path)
      .into_iter()
      .filter_map(Result::ok)
      .filter(|entry| {
        entry.file_type().is_file()
          && entry.path().extension().map_or(false, |ext| ext == "rs")
      });

    for entry in entries {
      let file_path = entry.path();

      let file_content = fs::read_to_string(file_path)?;

      let syntax = parse_file(&file_content)?;

      let module_name = file_path
        .strip_prefix(&src_path)?
        .to_string_lossy()
        .into_owned();

      let module_id = self.graph.nodes.len();

      let module_node = Node {
        id: module_id,
        name: module_name,
        kind: NodeKind::Module {
          path: file_path.to_path_buf(),
        },
        children: Vec::new(),
        documentation: String::new(),
        source_code: file_content,
      };

      self.graph.nodes.push(module_node);
      self.graph.nodes[parent_id].children.push(module_id);

      self.handle_syntactic_items(&syntax.items, file_path, module_id)?;
    }

    Ok(())
  }

  fn handle_syntactic_items(
    &mut self,
    items: &[Item],
    file_path: &Path,
    parent_id: NodeId,
  ) -> Result {
    for item in items {
      let source_code = item.to_token_stream().to_string();

      // tracing::info!("Processing item: {}", source_code);

      let node_id = self.graph.nodes.len();

      let mut node = Node {
        id: node_id,
        name: String::new(),
        kind: NodeKind::Unknown,
        children: Vec::new(),
        documentation: String::new(),
        source_code,
      };

      match item {
        Item::Const(i) => {
          node.name = i.ident.to_string();
          node.kind = NodeKind::Const {
            ty: i.ty.to_token_stream().to_string(),
            value: i.expr.to_token_stream().to_string(),
          };
        }
        Item::Enum(i) => {
          node.name = i.ident.to_string();
          node.kind = NodeKind::Enum {
            variants: i.variants.iter().map(|v| v.ident.to_string()).collect(),
          };
        }
        Item::Fn(i) => {
          node.name = i.sig.ident.to_string();
          node.kind = NodeKind::Function {
            arguments: i
              .sig
              .inputs
              .iter()
              .filter_map(|arg| {
                if let FnArg::Typed(pat_type) = arg {
                  Some(Field {
                    name: pat_type.pat.to_token_stream().to_string(),
                    type_name: pat_type.ty.to_token_stream().to_string(),
                  })
                } else {
                  None
                }
              })
              .collect(),
            return_type: match &i.sig.output {
              ReturnType::Default => None,
              ReturnType::Type(_, ty) => Some(ty.to_token_stream().to_string()),
            },
          };
        }
        Item::Macro(i) => {
          node.name = i
            .ident
            .as_ref()
            .map_or("macro".to_string(), |ident| ident.to_string());
          node.kind = NodeKind::Macro {
            macro_rules: i.mac.path.is_ident("macro_rules"),
          };
        }
        Item::Macro2(i) => {
          node.name = i.ident.to_string();
          node.kind = NodeKind::Macro { macro_rules: false };
        }
        Item::Mod(i) => {
          node.name = i.ident.to_string();
          node.kind = NodeKind::Module {
            path: file_path.to_path_buf(),
          };
          if let Some((_, items)) = &i.content {
            self.handle_syntactic_items(items, file_path, node_id)?;
          }
        }
        Item::Static(i) => {
          node.name = i.ident.to_string();
          node.kind = NodeKind::Static {
            ty: i.ty.to_token_stream().to_string(),
            mutability: i.mutability.is_some(),
          };
        }
        Item::Struct(i) => {
          node.name = i.ident.to_string();
          node.kind = NodeKind::Struct {
            fields: Self::handle_struct_fields(i),
          };
        }
        Item::Trait(i) => {
          node.name = i.ident.to_string();
          node.kind = NodeKind::Trait {
            is_auto: i.auto_token.is_some(),
            is_unsafe: i.unsafety.is_some(),
          };
        }
        Item::TraitAlias(i) => {
          node.name = i.ident.to_string();
          node.kind = NodeKind::TraitAlias {
            generics: i.generics.to_token_stream().to_string(),
          };
        }
        Item::Type(i) => {
          node.name = i.ident.to_string();
          node.kind = NodeKind::Type {
            generics: i.generics.to_token_stream().to_string(),
          };
        }
        _ => continue,
      }

      self.graph.nodes.push(node);
      self.graph.nodes[parent_id].children.push(node_id);

      self.trace_dependencies(item, node_id, parent_id);
    }

    Ok(())
  }

  fn trace_dependencies(
    &mut self,
    item: &Item,
    current_id: NodeId,
    current_module_id: NodeId,
  ) {
    let mut visitor =
      DependencyVisitor::new(&mut self.graph, current_id, current_module_id);

    match item {
      Item::Const(i) => visitor.visit_item_const(i),
      Item::Enum(i) => visitor.visit_item_enum(i),
      Item::ExternCrate(i) => visitor.visit_item_extern_crate(i),
      Item::Fn(i) => visitor.visit_item_fn(i),
      Item::ForeignMod(i) => visitor.visit_item_foreign_mod(i),
      Item::Impl(i) => visitor.visit_item_impl(i),
      Item::Mod(i) => visitor.visit_item_mod(i),
      Item::Static(i) => visitor.visit_item_static(i),
      Item::Struct(i) => visitor.visit_item_struct(i),
      Item::Trait(i) => visitor.visit_item_trait(i),
      Item::TraitAlias(i) => visitor.visit_item_trait_alias(i),
      Item::Type(i) => visitor.visit_item_type(i),
      Item::Union(i) => visitor.visit_item_union(i),
      Item::Use(i) => visitor.visit_item_use(i),
      _ => {}
    }
  }

  fn handle_struct_fields(item_struct: &ItemStruct) -> Vec<Field> {
    match item_struct {
      ItemStruct {
        fields: Fields::Named(named_fields),
        ..
      } => named_fields
        .named
        .iter()
        .map(|field| Field {
          name: field.ident.as_ref().unwrap().to_string(),
          type_name: field.ty.to_token_stream().to_string(),
        })
        .collect(),
      _ => Vec::new(),
    }
  }
}

struct DependencyVisitor<'a> {
  graph: &'a mut Graph,
  current_id: NodeId,
  current_module_id: NodeId,
}

impl<'a> DependencyVisitor<'a> {
  fn new(
    graph: &'a mut Graph,
    current_id: NodeId,
    current_module_id: NodeId,
  ) -> Self {
    Self {
      graph,
      current_id,
      current_module_id,
    }
  }

  fn find_node_by_name(&self, name: &str) -> Option<NodeId> {
    self.graph.nodes.iter().position(|node| node.name == name)
  }

  fn find_node_in_module(
    &self,
    module_id: NodeId,
    name: &str,
  ) -> Option<NodeId> {
    self.graph.nodes[module_id]
      .children
      .iter()
      .find(|&&child_id| self.graph.nodes[child_id].name == name)
      .cloned()
  }

  fn add_dependency(&mut self, target_id: NodeId) {
    if !self.graph.nodes[self.current_id]
      .children
      .contains(&target_id)
    {
      self.graph.nodes[self.current_id].children.push(target_id);
    }
  }
}

impl<'ast> Visit<'ast> for DependencyVisitor<'_> {
  fn visit_path(&mut self, path: &'ast syn::Path) {
    if let Some(ident) = path.get_ident() {
      let name = ident.to_string();

      if let Some(target_id) =
        self.find_node_in_module(self.current_module_id, &name)
      {
        self.add_dependency(target_id);
      } else {
        if let Some(target_id) = self.find_node_by_name(&name) {
          self.add_dependency(target_id);
        }
      }
    } else {
      let mut current_module_id = self.current_module_id;

      for segment in path.segments.iter() {
        let name = segment.ident.to_string();
        if let Some(target_id) =
          self.find_node_in_module(current_module_id, &name)
        {
          self.add_dependency(target_id);
          current_module_id = target_id;
        } else {
          break;
        }
      }
    }

    syn::visit::visit_path(self, path);
  }

  fn visit_item(&mut self, i: &'ast syn::Item) {
    syn::visit::visit_item(self, i);
  }

  fn visit_type(&mut self, ty: &'ast syn::Type) {
    syn::visit::visit_type(self, ty);
  }

  fn visit_expr(&mut self, expr: &'ast syn::Expr) {
    syn::visit::visit_expr(self, expr);
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
    let mut analyzer = Analyzer::new();

    match analyzer.analyze(&options.crate_path) {
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
