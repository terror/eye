import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet';
import { Highlight, themes } from 'prism-react-renderer';
import React, { useCallback, useEffect, useMemo, useState } from 'react';
import VisGraph, {
  Edge,
  GraphData,
  Node,
  Options,
} from 'react-vis-graph-wrapper';

interface RawNode {
  id: number;
  name: string;
  kind: NodeKind;
  children: number[];
  documentation: string;
  sourceCode: string;
}

type NodeKind =
  | { type: 'workspace'; content: { path: string } }
  | { type: 'package'; content: { path: string } }
  | { type: 'module'; content: { path: string } }
  | { type: 'struct'; content: { fields: Field[] } }
  | { type: 'enum'; content: { variants: string[] } }
  | {
      type: 'function';
      content: { arguments: Field[]; returnType: string | null };
    }
  | { type: 'const'; content: { ty: string; value: string } }
  | { type: 'macro'; content: { macroRules: boolean } }
  | { type: 'static'; content: { ty: string; mutability: boolean } }
  | { type: 'trait'; content: { isAuto: boolean; isUnsafe: boolean } }
  | { type: 'traitAlias'; content: { generics: string } }
  | { type: 'type'; content: { generics: string } }
  | { type: 'unknown' };

interface Field {
  name: string;
  typeName: string;
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
  const [isSheetOpen, setIsSheetOpen] = useState(false);

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

  const transformData = useCallback((data: RawGraph): CustomGraphData => {
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
  }, []);

  const getNodeLabel = useCallback((node: RawNode): string => {
    if (
      node.kind.type === 'module' ||
      node.kind.type === 'package' ||
      node.kind.type === 'workspace'
    ) {
      return node.name.split('/').pop() || node.name;
    }
    return node.name;
  }, []);

  const generateNodeTooltip = useCallback((node: RawNode): string => {
    let details = '';

    switch (node.kind.type) {
      case 'workspace':
        details = `Workspace`;
        break;
      case 'package':
        details = `Package`;
        break;
      case 'module':
        details = `Module`;
        break;
      case 'struct':
        details = `Struct with ${node.kind.content.fields.length} fields`;
        break;
      case 'enum':
        details = `Enum with ${node.kind.content.variants.length} variants`;
        break;
      case 'function':
        details = `Function with ${node.kind.content.arguments.length} arguments`;
        break;
      case 'const':
        details = `Constant`;
        break;
      case 'macro':
        details = `Macro`;
        break;
      case 'static':
        details = `Static`;
        break;
      case 'trait':
        details = `Trait`;
        break;
      case 'traitAlias':
        details = `Trait Alias`;
        break;
      case 'type':
        details = `Type`;
        break;
      case 'unknown':
        details = `Unknown`;
        break;
    }
    return details;
  }, []);

  const getNodeColor = useCallback((kind: NodeKind): string => {
    switch (kind.type) {
      case 'workspace':
        return '#FF6B6B';
      case 'package':
        return '#4ECDC4';
      case 'module':
        return '#97C2FC';
      case 'struct':
        return '#FFCCCB';
      case 'enum':
        return '#90EE90';
      case 'function':
        return '#FFD700';
      case 'const':
        return '#FFA07A';
      case 'macro':
        return '#FF69B4';
      case 'static':
        return '#8FBC8F';
      case 'trait':
        return '#ADD8E6';
      case 'traitAlias':
        return '#E6E6FA';
      case 'type':
        return '#F08080';
      case 'unknown':
        return '#D3D3D3';
      default:
        return '#D3D3D3';
    }
  }, []);

  useEffect(() => {
    setIsSheetOpen(!!selectedNode);
  }, [selectedNode]);

  const options: Options = useMemo(
    () => ({
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
    }),
    [graphHeight]
  );

  const handleNodeSelect = useCallback(
    (event: { nodes: number[] }) => {
      const { nodes } = event;
      const selected =
        graph?.nodes.find((node) => node.id === nodes[0]) || null;
      console.log('Node selected:', selected);
      setSelectedNode(selected);
      setIsSheetOpen(true);
    },
    [graph]
  );

  const handleSheetOpenChange = useCallback((open: boolean) => {
    console.log('Sheet open state changed to:', open);
    setIsSheetOpen(open);
    if (!open) {
      setSelectedNode(null);
    }
  }, []);

  const events = useMemo(
    () => ({
      select: handleNodeSelect,
    }),
    [handleNodeSelect]
  );

  const renderNodeDetails = useCallback(() => {
    if (!selectedNode) return null;

    const { rawData } = selectedNode;

    return (
      <div className='space-y-4'>
        <h2 className='text-xl font-bold'>{selectedNode.label}</h2>
        <p>
          <strong>Type:</strong> {rawData.kind.type}
        </p>
        {(rawData.kind.type === 'workspace' ||
          rawData.kind.type === 'package' ||
          rawData.kind.type === 'module') && (
          <p>
            <strong>Path:</strong> {rawData.kind.content.path}
          </p>
        )}
        {rawData.kind.type === 'struct' && (
          <div>
            <p>
              <strong>Fields:</strong>
            </p>
            <ul className='list-disc pl-5'>
              {rawData.kind.content.fields.map(
                (field: Field, index: number) => (
                  <li key={index}>
                    {field.name}: {field.typeName}
                  </li>
                )
              )}
            </ul>
          </div>
        )}
        {rawData.kind.type === 'enum' && (
          <div>
            <p>
              <strong>Variants:</strong>
            </p>
            <ul className='list-disc pl-5'>
              {rawData.kind.content.variants.map(
                (variant: string, index: number) => (
                  <li key={index}>{variant}</li>
                )
              )}
            </ul>
          </div>
        )}
        {rawData.kind.type === 'function' && (
          <div>
            <p>
              <strong>Arguments:</strong>
            </p>
            <ul className='list-disc pl-5'>
              {rawData.kind.content.arguments.map(
                (arg: Field, index: number) => (
                  <li key={index}>
                    {arg.name}: {arg.typeName}
                  </li>
                )
              )}
            </ul>
            <p>
              <strong>Return Type:</strong>{' '}
              {rawData.kind.content.returnType || 'None'}
            </p>
          </div>
        )}
        {rawData.kind.type === 'const' && (
          <div>
            <p>
              <strong>Type:</strong> {rawData.kind.content.ty}
            </p>
            <p>
              <strong>Value:</strong> {rawData.kind.content.value}
            </p>
          </div>
        )}
        {rawData.kind.type === 'macro' && (
          <p>
            <strong>Macro Rules:</strong>{' '}
            {rawData.kind.content.macroRules ? 'Yes' : 'No'}
          </p>
        )}
        {rawData.kind.type === 'static' && (
          <div>
            <p>
              <strong>Type:</strong> {rawData.kind.content.ty}
            </p>
            <p>
              <strong>Mutability:</strong>{' '}
              {rawData.kind.content.mutability ? 'Mutable' : 'Immutable'}
            </p>
          </div>
        )}
        {rawData.kind.type === 'trait' && (
          <div>
            <p>
              <strong>Auto:</strong>{' '}
              {rawData.kind.content.isAuto ? 'Yes' : 'No'}
            </p>
            <p>
              <strong>Unsafe:</strong>{' '}
              {rawData.kind.content.isUnsafe ? 'Yes' : 'No'}
            </p>
          </div>
        )}
        {rawData.kind.type === 'traitAlias' && (
          <p>
            <strong>Generics:</strong> {rawData.kind.content.generics}
          </p>
        )}
        {rawData.kind.type === 'type' && (
          <p>
            <strong>Generics:</strong> {rawData.kind.content.generics}
          </p>
        )}
        {rawData.documentation && (
          <div>
            <p>
              <strong>Documentation:</strong>
            </p>
            <p>{rawData.documentation}</p>
          </div>
        )}
        {rawData.sourceCode && (
          <div>
            <p>
              <strong>Source Code:</strong>
            </p>
            <div className='max-h-96 overflow-auto'>
              <Highlight
                theme={themes.github}
                code={rawData.sourceCode}
                language='rust'
              >
                {({ className, style, tokens }) => (
                  <pre
                    className={className}
                    style={{
                      ...style,
                      padding: '1rem',
                      borderRadius: '0.5rem',
                    }}
                  >
                    {tokens.map((line, i) => (
                      <div key={i}>
                        {line.map((token) => token.content).join('')}
                      </div>
                    ))}
                  </pre>
                )}
              </Highlight>
            </div>
          </div>
        )}
      </div>
    );
  }, [selectedNode]);

  return (
    <div className='relative h-screen w-screen overflow-hidden'>
      <div className='absolute inset-0 pr-[400px]'>
        {graph && (
          <VisGraph
            graph={graph}
            options={options}
            events={events}
            style={{ height: '100%', width: '100%' }}
          />
        )}
      </div>
      <Sheet open={isSheetOpen} onOpenChange={handleSheetOpenChange}>
        <SheetContent
          side='right'
          className='w-[400px] overflow-y-auto sm:w-[540px]'
        >
          <SheetHeader>
            <SheetTitle>{selectedNode?.label || 'Node Details'}</SheetTitle>
            <SheetDescription>
              {selectedNode
                ? `Details for ${selectedNode.rawData.kind.type}`
                : 'Select a node to view details'}
            </SheetDescription>
          </SheetHeader>
          <div className='py-4'>
            {selectedNode ? (
              renderNodeDetails()
            ) : (
              <p>Select a node to view its details.</p>
            )}
          </div>
        </SheetContent>
      </Sheet>
    </div>
  );
};

const App: React.FC = () => {
  return <Graph />;
};

export default App;
