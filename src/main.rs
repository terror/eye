use clap::Parser;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use syn::__private::ToTokens;
use syn::{parse_file, Fields, Item, ItemStruct};
use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
  /// Path to the Rust crate to analyze
  #[clap(short, long, value_parser)]
  crate_path: PathBuf,
}

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

fn analyze(crate_path: &Path) -> Result<Graph, Box<dyn std::error::Error>> {
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
    documentation: String::new(), // TODO: Generate documentation
  };
  graph.nodes.push(root_node);

  process_crate(&mut graph, crate_path, 0)?;

  Ok(graph)
}

fn process_crate(
  graph: &mut Graph,
  crate_path: &Path,
  parent_id: NodeId,
) -> Result<(), Box<dyn std::error::Error>> {
  for entry in WalkDir::new(crate_path).into_iter().filter_map(Result::ok) {
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
        documentation: String::new(), // TODO: Generate documentation
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
) -> Result<(), Box<dyn std::error::Error>> {
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
            documentation: String::new(), // TODO: Generate documentation
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
          documentation: String::new(), // TODO: Generate documentation
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
          documentation: String::new(), // TODO: Generate documentation
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
          documentation: String::new(), // TODO: Generate documentation
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let args = Args::parse();
  let crate_graph = analyze(&args.crate_path)?;

  // Serialize to JSON
  let json = serde_json::to_string_pretty(&crate_graph)?;
  fs::write("crate_graph.json", json)?;

  println!("Crate graph has been written to crate_graph.json");
  Ok(())
}
