import { useEffect, useRef, useState } from "react";
import {
  forceCenter,
  forceCollide,
  forceLink,
  forceManyBody,
  forceSimulation,
  type SimulationLinkDatum,
  type SimulationNodeDatum,
} from "d3-force";
import { api } from "../api";

interface Node extends SimulationNodeDatum {
  id: string;
  vault: string;
  label: string;
}

type Edge = SimulationLinkDatum<Node>;

const VAULT_COLORS = ["--v1", "--v2", "--v3", "--v4", "--v5", "--v6"];

interface Props {
  vault: string;
  vaults: string[];
  onOpen: (vault: string, title: string) => void;
}

export function GraphView({ vault, vaults, onOpen }: Props) {
  const [allVaults, setAllVaults] = useState(vaults.length > 1);

  // The vault list arrives async; when it does, default to the combined view
  // (covers landing directly on ?view=graph before vaults have loaded).
  useEffect(() => {
    setAllVaults(vaults.length > 1);
  }, [vaults.length]);
  const [nodes, setNodes] = useState<Node[]>([]);
  const [edges, setEdges] = useState<Edge[]>([]);
  const [zoom, setZoom] = useState(1);
  const simRef = useRef<ReturnType<typeof forceSimulation<Node>> | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  const colorOf = (vaultName: string) => {
    const index = Math.max(0, vaults.indexOf(vaultName)) % VAULT_COLORS.length;
    return `var(${VAULT_COLORS[index]})`;
  };

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      const data = await api.graph(allVaults ? undefined : vault);
      if (cancelled) return;

      const parse = (id: string): { vault: string; label: string } => {
        if (allVaults) {
          const slash = id.indexOf("/");
          if (slash > 0 && vaults.includes(id.slice(0, slash))) {
            return { vault: id.slice(0, slash), label: id.slice(slash + 1) };
          }
        }
        return { vault, label: id };
      };

      const width = containerRef.current?.clientWidth ?? 800;
      const height = containerRef.current?.clientHeight ?? 600;
      const nodeList: Node[] = data.nodes.map((id) => ({
        id,
        ...parse(id),
        x: width / 2 + (Math.random() - 0.5) * 200,
        y: height / 2 + (Math.random() - 0.5) * 200,
      }));
      const byId = new Map(nodeList.map((n) => [n.id, n]));
      const edgeList: Edge[] = data.edges
        .filter((e) => byId.has(e.from) && byId.has(e.to))
        .map((e) => ({ source: e.from, target: e.to }));

      simRef.current?.stop();
      const simulation = forceSimulation(nodeList)
        .force("charge", forceManyBody().strength(-220))
        .force(
          "link",
          forceLink<Node, Edge>(edgeList)
            .id((n) => n.id)
            .distance(90),
        )
        .force("center", forceCenter(width / 2, height / 2))
        .force("collide", forceCollide(26));
      simulation.on("tick", () => {
        setNodes([...nodeList]);
        setEdges([...edgeList]);
      });
      simRef.current = simulation;
    })();
    return () => {
      cancelled = true;
      simRef.current?.stop();
    };
  }, [vault, allVaults, vaults]);

  // Basic node dragging: pin while dragging, release after.
  const dragging = useRef<Node | null>(null);
  const onPointerDown = (node: Node) => (e: React.PointerEvent) => {
    e.preventDefault();
    dragging.current = node;
    simRef.current?.alphaTarget(0.25).restart();
  };
  const onPointerMove = (e: React.PointerEvent<SVGSVGElement>) => {
    const node = dragging.current;
    if (!node) return;
    const rect = e.currentTarget.getBoundingClientRect();
    node.fx = (e.clientX - rect.left) / zoom;
    node.fy = (e.clientY - rect.top) / zoom;
  };
  const endDrag = () => {
    if (dragging.current) {
      dragging.current.fx = null;
      dragging.current.fy = null;
      dragging.current = null;
      simRef.current?.alphaTarget(0);
    }
  };

  const usedVaults = allVaults
    ? vaults.filter((v) => nodes.some((n) => n.vault === v))
    : [vault];

  return (
    <div className="graph-view" ref={containerRef}>
      <svg
        onPointerMove={onPointerMove}
        onPointerUp={endDrag}
        onPointerLeave={endDrag}
        onWheel={(e) =>
          setZoom((z) => Math.min(3, Math.max(0.3, z * (e.deltaY < 0 ? 1.12 : 0.9))))
        }
        role="img"
        aria-label="กราฟความเชื่อมโยงของโน้ต"
      >
        <g transform={`scale(${zoom})`}>
          {edges.map((edge, i) => {
            const s = edge.source as Node;
            const t = edge.target as Node;
            return (
              <line
                key={i}
                className="graph-edge"
                x1={s.x}
                y1={s.y}
                x2={t.x}
                y2={t.y}
              />
            );
          })}
          {nodes.map((node) => (
            <g key={node.id} className="graph-node">
              <circle
                cx={node.x}
                cy={node.y}
                r={9}
                fill={colorOf(node.vault)}
                onPointerDown={onPointerDown(node)}
                onClick={() => onOpen(node.vault, node.label)}
              >
                <title>{node.id}</title>
              </circle>
              <text x={(node.x ?? 0) + 13} y={(node.y ?? 0) + 4}>
                {node.label}
              </text>
            </g>
          ))}
        </g>
      </svg>

      <div className="graph-legend">
        {usedVaults.map((v) => (
          <span key={v}>
            <span className="dot" style={{ background: colorOf(v) }} />
            {v}
          </span>
        ))}
      </div>

      <div className="graph-controls">
        {vaults.length > 1 && (
          <button
            className={`btn ${allVaults ? "active" : ""}`}
            onClick={() => setAllVaults(!allVaults)}
          >
            ทุก vault
          </button>
        )}
      </div>
    </div>
  );
}
