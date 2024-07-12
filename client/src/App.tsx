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
  rawData: RawNode;
}

interface CustomGraphData extends GraphData {
  nodes: CustomNode[];
}

const Graph: React.FC = () => {
  const [graph, setGraph] = useState<CustomGraphData | null>(null);
  const [selectedNode, setSelectedNode] = useState<CustomNode | null>(null);
  const [graphHeight, setGraphHeight] = useState<string>('100vh');

  useEffect(() => {
    fetch('/api/graph')
      .then((response) => response.json())
      .then((data: RawGraph) => {
        const graphData = transformData(data);
        setGraph(graphData);
      })
      .catch((error) => console.error('Error fetching graph data:', error));

    const updateHeight = () => {
      setGraphHeight(`${window.innerHeight}px`);
    };

    window.addEventListener('resize', updateHeight);
    updateHeight();

    return () => window.removeEventListener('resize', updateHeight);
  }, []);

  const transformData = (data: RawGraph): CustomGraphData => {
    const nodes: CustomNode[] = data.nodes.map((node) => ({
      id: node.id,
      label: getNodeLabel(node),
      title: generateNodeTooltip(node),
      color: getNodeColor(node.kind),
      rawData: node,
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

  const getNodeLabel = (node: RawNode): string => {
    if (node.kind.module) {
      return node.name.split('/').pop() || node.name;
    }
    return node.name;
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
    return `<strong>${node.name}</strong><br>${details}`;
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
        direction: 'LR',
        sortMethod: 'directed',
        levelSeparation: 150,
        nodeSpacing: 150,
      },
    },
    edges: {
      color: '#000000',
    },
    height: graphHeight,
    width: '100%',
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

  const renderNodeDetails = (node: CustomNode) => {
    const { rawData } = node;
    return (
      <div>
        <h2 className='text-xl font-bold'>{node.label}</h2>
        <p>
          <strong>Type:</strong> {Object.keys(rawData.kind)[0]}
        </p>
        {rawData.kind.module && (
          <p>
            <strong>Path:</strong> {rawData.kind.module.path}
          </p>
        )}
        {rawData.kind.struct && (
          <div>
            <p>
              <strong>Fields:</strong>
            </p>
            <ul>
              {rawData.kind.struct.fields.map((field: any, index: number) => (
                <li key={index}>
                  {field.name}: {field.type_name}
                </li>
              ))}
            </ul>
          </div>
        )}
        {rawData.kind.enum && (
          <div>
            <p>
              <strong>Variants:</strong>
            </p>
            <ul>
              {rawData.kind.enum.variants.map(
                (variant: string, index: number) => (
                  <li key={index}>{variant}</li>
                )
              )}
            </ul>
          </div>
        )}
        {rawData.kind.function && (
          <div>
            <p>
              <strong>Arguments:</strong>
            </p>
            <ul>
              {rawData.kind.function.arguments.map(
                (arg: any, index: number) => (
                  <li key={index}>
                    {arg.name}: {arg.type_name}
                  </li>
                )
              )}
            </ul>
            <p>
              <strong>Return Type:</strong>{' '}
              {rawData.kind.function.return_type || 'None'}
            </p>
          </div>
        )}
        {rawData.documentation && (
          <div>
            <p>
              <strong>Documentation:</strong>
            </p>
            <p>{rawData.documentation}</p>
          </div>
        )}
      </div>
    );
  };

  return (
    <div className='flex h-screen'>
      <div className='flex-grow'>
        {graph && (
          <VisGraph
            graph={graph}
            options={options}
            events={events}
            style={{ height: graphHeight, width: '100%' }}
          />
        )}
      </div>
      {selectedNode && (
        <div className='w-1/4 overflow-auto border-l p-4'>
          {renderNodeDetails(selectedNode)}
        </div>
      )}
    </div>
  );
};

const App: React.FC = () => {
  return <Graph />;
};

export default App;
