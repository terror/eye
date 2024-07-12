import React, { useEffect, useState } from 'react';
import VisGraph, {
  Edge,
  GraphData,
  Node,
  Options,
} from 'react-vis-graph-wrapper';

interface RawNode {
  id: number;
  name: string;
  kind: {
    module?: { path: string };
    struct?: { fields: any[] };
    enum?: { variants: string[] };
    function?: { arguments: any[]; return_type: string | null };
  };
  children: number[];
  documentation: string;
}

interface RawGraph {
  root: number;
  nodes: RawNode[];
}

interface CustomNode extends Node {
  title: string;
}

interface CustomGraphData extends GraphData {
  nodes: CustomNode[];
}

const Graph: React.FC = () => {
  const [graph, setGraph] = useState<CustomGraphData | null>(null);
  const [selectedNode, setSelectedNode] = useState<CustomNode | null>(null);

  useEffect(() => {
    fetch('/api/graph')
      .then((response) => response.json())
      .then((data: RawGraph) => {
        const graphData = transformData(data);
        setGraph(graphData);
      })
      .catch((error) => console.error('Error fetching graph data:', error));
  }, []);

  const transformData = (data: RawGraph): CustomGraphData => {
    const nodes: CustomNode[] = data.nodes.map((node) => ({
      id: node.id,
      label: node.name,
      title: generateNodeTooltip(node),
      color: getNodeColor(node.kind),
    }));

    const edges: Edge[] = data.nodes.flatMap((node) =>
      node.children.map((childId) => ({
        from: node.id,
        to: childId,
        arrows: 'to',
      }))
    );

    return { nodes, edges };
  };

  const generateNodeTooltip = (node: RawNode): string => {
    let details = '';
    if (node.kind.struct) {
      details = `Struct with ${node.kind.struct.fields.length} fields`;
    } else if (node.kind.enum) {
      details = `Enum with ${node.kind.enum.variants.length} variants`;
    } else if (node.kind.function) {
      details = `Function with ${node.kind.function.arguments.length} arguments`;
    } else if (node.kind.module) {
      details = `Module`;
    }
    return `<strong>${node.name}</strong><br>${details}<br>${node.documentation}`;
  };

  const getNodeColor = (kind: RawNode['kind']): string => {
    if ('module' in kind) return '#97C2FC';
    if ('struct' in kind) return '#FFCCCB';
    if ('enum' in kind) return '#90EE90';
    if ('function' in kind) return '#FFD700';
    return '#D3D3D3';
  };

  const options: Options = {
    layout: {
      hierarchical: {
        direction: 'UD',
        sortMethod: 'directed',
      },
    },
    edges: {
      color: '#000000',
    },
    height: '500px',
    physics: {
      enabled: false,
    },
    interaction: {
      navigationButtons: true,
      keyboard: true,
    },
  };

  const events = {
    select: function (event: { nodes: number[] }) {
      const { nodes } = event;
      setSelectedNode(
        graph?.nodes.find((node) => node.id === nodes[0]) || null
      );
    },
  };

  return (
    <div className="relative">
      {graph && (
        <VisGraph
          graph={graph}
          options={options}
          events={events}
          style={{ height: '500px' }}
        />
      )}
      {selectedNode && (
        <div className='m-4 rounded p-4'>
          <h2 className='text-xl font-bold'>{selectedNode.label}</h2>
          <p dangerouslySetInnerHTML={{ __html: selectedNode.title }}></p>
        </div>
      )}
    </div>
  );
};

const App: React.FC = () => {
  return <Graph />;
};

export default App;
