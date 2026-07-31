import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  forceCenter,
  forceCollide,
  forceLink,
  forceManyBody,
  forceSimulation,
  forceX,
  forceY,
  type Simulation,
  type SimulationLinkDatum,
  type SimulationNodeDatum,
} from "d3-force";
import { api, type GraphData } from "../api";
import { useT } from "../i18n";

/**
 * The graph is the workspace, not a novelty tab — so it has to survive a real
 * vault. A project that pulls in vendored documentation reaches several hundred
 * notes, which is past what SVG can animate; this paints to canvas instead, and
 * gets dimming and glow along the way.
 *
 * Deliberately not WebGL: canvas handles thousands of nodes at 60fps, while a 3D
 * engine would add hundreds of kilobytes to a binary meant to ship as one file,
 * and buy nothing for the job of finding a note.
 */

interface Node extends SimulationNodeDatum {
  id: string;
  /** Vault-relative path — what opening the note needs. */
  key: string;
  vault: string;
  label: string;
  /** Wikilink target with no note behind it. */
  missing: boolean;
  reference: boolean;
  /** The folder the note sits in, used to cluster the layout. */
  group: string;
  degree: number;
}

type Edge = SimulationLinkDatum<Node>;

interface Props {
  vault: string;
  vaults: string[];
  allVaults: boolean;
  /** Keys that match the current search; null when no search is active. */
  matched: Set<string> | null;
  selectedKey: string;
  onSelect: (vault: string, key: string) => void;
  /** Bumped by the caller to force a refetch after notes change. */
  revision: number;
}

/** Vault hues, in fixed order. Validated for colour-vision deficiency and for
 *  contrast against both surfaces — see PLAN.md. Never cycled: a fifth vault
 *  falls back to neutral rather than reusing a hue that means another vault. */
const VAULT_HUES = ["#4a7fd4", "#c47b16", "#a35fc4", "#0f9b7e"];
const NEUTRAL_HUE = "#7c8598";

const DIMMED = 0.1;

export function GraphCanvas({
  vault,
  vaults,
  allVaults,
  matched,
  selectedKey,
  onSelect,
  revision,
}: Props) {
  const t = useT();
  const wrapRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const simRef = useRef<Simulation<Node, Edge> | null>(null);
  const nodesRef = useRef<Node[]>([]);
  const edgesRef = useRef<Edge[]>([]);
  const cameraRef = useRef({ x: 0, y: 0, k: 1 });
  const hoverRef = useRef<Node | null>(null);
  const dragRef = useRef<Node | null>(null);
  const panRef = useRef<{ x: number; y: number } | null>(null);
  const frameRef = useRef(0);
  /**
   * Set the moment the reader pans, zooms or drags a node.
   *
   * Auto-framing runs on every tick until then, which is what makes it reliable:
   * fitting once when the simulation reports "end" measured a layout that was
   * still contracting, so the camera locked to a spread the graph no longer had
   * and the map came out smaller than before. After the first deliberate move the
   * camera belongs to the reader and nothing may override it.
   */
  const userMovedRef = useRef(false);

  const [hovered, setHovered] = useState<{ node: Node; x: number; y: number } | null>(null);
  const [counts, setCounts] = useState({ nodes: 0, edges: 0, hidden: 0 });
  /**
   * Reference notes are off by default.
   *
   * A vault that pulls in vendored documentation can hold 425 read-only notes
   * against 5 of your own, and at that ratio the graph stops being a map: it is a
   * uniform field of rings with your actual knowledge lost inside it. The map
   * answers "what do I know" — somebody else's docs are a lookup resource, and
   * they are one click away when you want them.
   */
  const [showReference, setShowReference] = useState(false);

  const colorOf = useCallback(
    (node: Node) => {
      if (!allVaults) return VAULT_HUES[0];
      const index = vaults.indexOf(node.vault);
      return index >= 0 && index < VAULT_HUES.length ? VAULT_HUES[index] : NEUTRAL_HUE;
    },
    [allVaults, vaults],
  );

  /** Neighbours of the selection, so choosing a note also shows its context. */
  const neighbours = useMemo(() => {
    const set = new Set<string>();
    if (!selectedKey) return set;
    for (const edge of edgesRef.current) {
      const s = edge.source as Node;
      const t = edge.target as Node;
      if (!s || !t) continue;
      if (s.key === selectedKey) set.add(t.key);
      if (t.key === selectedKey) set.add(s.key);
    }
    return set;
    // Recomputed on every paint-relevant change; edges live in a ref.
  }, [selectedKey, counts.edges]);

  const paint = useCallback(() => {
    const canvas = canvasRef.current;
    const ctx = canvas?.getContext("2d");
    if (!canvas || !ctx) return;

    const styles = getComputedStyle(document.documentElement);
    const inkMute = styles.getPropertyValue("--ink-mute").trim() || "#888";
    const lineColor = styles.getPropertyValue("--line-strong").trim() || "#555";
    const found = styles.getPropertyValue("--found").trim() || "#d8f24a";
    const ink = styles.getPropertyValue("--ink").trim() || "#fff";

    const dpr = window.devicePixelRatio || 1;
    const width = canvas.clientWidth;
    const height = canvas.clientHeight;
    if (canvas.width !== width * dpr || canvas.height !== height * dpr) {
      canvas.width = width * dpr;
      canvas.height = height * dpr;
    }
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, width, height);

    const cam = cameraRef.current;
    ctx.translate(width / 2 + cam.x, height / 2 + cam.y);
    ctx.scale(cam.k, cam.k);

    const alphaOf = (node: Node) => {
      if (matched && !matched.has(node.key)) return DIMMED;
      if (selectedKey && node.key !== selectedKey && !neighbours.has(node.key)) return 0.35;
      // When reference notes are shown they are ground, not figure: they still
      // outnumber your own notes by a lot, and at equal weight they are all you
      // see. A match or a selection overrides this — being found outranks being
      // background.
      if (node.reference) return 0.45;
      return 1;
    };

    // Edges first, so nodes sit on top.
    ctx.lineWidth = 1 / cam.k;
    for (const edge of edgesRef.current) {
      const s = edge.source as Node;
      const t = edge.target as Node;
      if (!s?.x || !t?.x) continue;
      const alpha = Math.min(alphaOf(s), alphaOf(t)) * 0.55;
      ctx.globalAlpha = alpha;
      ctx.strokeStyle = lineColor;
      ctx.beginPath();
      ctx.moveTo(s.x, s.y!);
      ctx.lineTo(t.x, t.y!);
      ctx.stroke();
    }

    const labelZoom = cam.k > 1.15;
    ctx.font = `500 ${11 / cam.k}px "IBM Plex Sans Thai", system-ui, sans-serif`;
    ctx.textBaseline = "middle";

    for (const node of nodesRef.current) {
      if (node.x === undefined || node.y === undefined) continue;
      const alpha = alphaOf(node);
      const isMatch = !!matched?.has(node.key);
      const isSelected = node.key === selectedKey;
      // Size carries the connection count: a hub looks like a hub. Reference
      // notes are drawn smaller as well as fainter — two channels saying the same
      // thing, because one was not enough at a ratio of 85 to 1.
      const radius = node.missing
        ? 3.5
        : node.reference
          ? 2.8 + Math.min(node.degree, 12) * 0.3
          : 4 + Math.min(node.degree, 12) * 0.75;

      ctx.globalAlpha = alpha;

      // Glow means "found" and nothing else — it is the one loud effect here.
      if (isMatch || isSelected) {
        ctx.shadowColor = found;
        ctx.shadowBlur = 18 / cam.k;
      } else {
        ctx.shadowBlur = 0;
      }

      ctx.beginPath();
      ctx.arc(node.x, node.y, radius, 0, Math.PI * 2);
      if (node.missing) {
        // A target with no note yet: hollow, so it never reads as content.
        ctx.strokeStyle = inkMute;
        ctx.lineWidth = 1.2 / cam.k;
        ctx.setLineDash([2 / cam.k, 2 / cam.k]);
        ctx.stroke();
        ctx.setLineDash([]);
      } else if (node.reference) {
        // Read-only knowledge from a dependency: outlined, not filled.
        ctx.strokeStyle = colorOf(node);
        ctx.lineWidth = 1.6 / cam.k;
        ctx.stroke();
      } else {
        ctx.fillStyle = colorOf(node);
        ctx.fill();
      }
      ctx.shadowBlur = 0;

      if (isSelected) {
        ctx.beginPath();
        ctx.arc(node.x, node.y, radius + 4 / cam.k, 0, Math.PI * 2);
        ctx.strokeStyle = found;
        ctx.lineWidth = 2 / cam.k;
        ctx.stroke();
      }

      const showLabel = isSelected || isMatch || hoverRef.current === node || labelZoom;
      if (showLabel) {
        ctx.globalAlpha = alpha;
        ctx.fillStyle = isSelected || isMatch ? ink : inkMute;
        ctx.fillText(node.label, node.x + radius + 5 / cam.k, node.y);
      }
    }
    ctx.globalAlpha = 1;
  }, [colorOf, matched, neighbours, selectedKey]);

  /**
   * Move the camera so every node is on screen, with room to breathe.
   *
   * Zooming *in* is the whole point for a small vault, so the upper clamp has to
   * be above 1 — capped at 2.5x because a three-note vault magnified to fill a
   * 27-inch display looks like a mistake. The lower bound lets a few hundred
   * notes shrink to fit rather than spill off the edges.
   */
  const fitToNodes = useCallback(() => {
    const canvas = canvasRef.current;
    const nodes = nodesRef.current.filter((n) => n.x !== undefined && n.y !== undefined);
    if (!canvas || nodes.length === 0) return;
    const width = canvas.clientWidth;
    const height = canvas.clientHeight;
    if (width === 0 || height === 0) return;

    let minX = Infinity;
    let minY = Infinity;
    let maxX = -Infinity;
    let maxY = -Infinity;
    for (const node of nodes) {
      minX = Math.min(minX, node.x!);
      minY = Math.min(minY, node.y!);
      maxX = Math.max(maxX, node.x!);
      maxY = Math.max(maxY, node.y!);
    }
    // Padding covers the node radius and its label, neither of which is in the
    // bounding box of the centres.
    const pad = 60;
    const spanX = Math.max(maxX - minX, 1) + pad * 2;
    const spanY = Math.max(maxY - minY, 1) + pad * 2;
    const k = Math.max(0.2, Math.min(2.5, Math.min(width / spanX, height / spanY)));
    // The paint transform is translate(centre + cam) then scale(k), so the offset
    // that centres the cloud is its midpoint scaled by k, negated.
    cameraRef.current = {
      k,
      x: -((minX + maxX) / 2) * k,
      y: -((minY + maxY) / 2) * k,
    };
    paintNow();
    // paintNow is stable; nodesRef/canvasRef are refs.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  /** Coalesce repaints while the layout is settling. */
  const schedulePaint = useCallback(() => {
    cancelAnimationFrame(frameRef.current);
    frameRef.current = requestAnimationFrame(paint);
  }, [paint]);

  /**
   * Paint right now, without waiting for a frame.
   *
   * A hidden tab gets no animation frames, so anything that only ever repaints
   * through `requestAnimationFrame` shows an empty canvas when the tab comes
   * back — the simulation has finished by then and will never tick again.
   * State changes and size changes therefore paint directly.
   */
  const paintNow = useCallback(() => {
    cancelAnimationFrame(frameRef.current);
    paint();
  }, [paint]);

  // ---- load + lay out ----
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      let data: GraphData;
      try {
        data = await api.graph(allVaults ? undefined : vault);
      } catch {
        return;
      }
      if (cancelled) return;

      const degree = new Map<string, number>();
      for (const edge of data.edges) {
        degree.set(edge.from, (degree.get(edge.from) ?? 0) + 1);
        degree.set(edge.to, (degree.get(edge.to) ?? 0) + 1);
      }

      /**
       * Your notes, plus one hop.
       *
       * Dropping every reference note outright was too blunt: this vault's only
       * two outgoing links go from a note of yours *to* a vendored doc page, and
       * its six "not created yet" targets are all named by reference notes. Cut
       * them all and the map is 11 unconnected dots — technically the right
       * filter, uselessly wrong as a picture.
       *
       * So: keep your own notes, and keep whatever they touch directly. A
       * borrowed page you actually cite is part of your map. A missing target is
       * only meaningful beside the note that names it, so it comes along only if
       * that note is here.
       */
      const keep = new Set<string>();
      if (showReference) {
        for (const n of data.nodes) keep.add(n.id);
      } else {
        for (const n of data.nodes) {
          if (!n.reference && !n.missing) keep.add(n.id);
        }
        // Snapshot the seed so this stays exactly one hop, not a flood fill.
        const seed = new Set(keep);
        for (const e of data.edges) {
          if (seed.has(e.from)) keep.add(e.to);
          if (seed.has(e.to)) keep.add(e.from);
        }
      }

      const visible = data.nodes.filter((n) => keep.has(n.id));
      // Counted against the legend entry it sits next to, so it says how many
      // read-only notes are off the map — not how many nodes of any kind are.
      const hidden = data.nodes.filter((n) => n.reference && !keep.has(n.id)).length;

      const nodes: Node[] = visible.map((n) => {
        let nodeVault = vault;
        let key = n.id;
        if (allVaults) {
          const slash = n.id.indexOf("/");
          if (slash > 0 && vaults.includes(n.id.slice(0, slash))) {
            nodeVault = n.id.slice(0, slash);
            key = n.id.slice(slash + 1);
          }
        }
        // The *deepest* folder, not the top-level one. Vendored docs all live
        // under `node_modules`, so a top-level key collapsed every reference note
        // into a single group and the clustering could not say anything. The
        // directory a note actually sits in is what distinguishes it.
        const slash = key.lastIndexOf("/");
        return {
          id: n.id,
          key,
          vault: nodeVault,
          label: n.label,
          missing: n.missing,
          reference: n.reference,
          group: allVaults ? nodeVault : slash > 0 ? key.slice(0, slash) : "",
          degree: degree.get(n.id) ?? 0,
        };
      });

      const byId = new Map(nodes.map((n) => [n.id, n]));
      const edges: Edge[] = data.edges
        .filter((e) => byId.has(e.from) && byId.has(e.to))
        .map((e) => ({ source: byId.get(e.from)!, target: byId.get(e.to)! }));

      // Anchor each folder (or vault) at its own point on a ring. Without this a
      // few hundred notes settle into one indistinguishable ball; with it the
      // layout shows the structure the paths already describe.
      // Only the populous folders get an anchor. Grouping by the deepest folder
      // can produce dozens of groups, and a ring of dozens of anchors is a ring,
      // not a structure — so the biggest clusters get a position and the long tail
      // is left to the centre, where it reads as "everything else".
      const population = new Map<string, number>();
      for (const node of nodes) {
        if (node.group) population.set(node.group, (population.get(node.group) ?? 0) + 1);
      }
      const groups = [...population.entries()]
        .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
        .slice(0, 10)
        .map(([name]) => name)
        .sort();
      const anchor = (node: Node) => {
        const index = groups.indexOf(node.group);
        if (index < 0) return { x: 0, y: 0 };
        const angle = (index / groups.length) * Math.PI * 2;
        const spread = groups.length > 1 ? 190 : 0;
        return { x: Math.cos(angle) * spread, y: Math.sin(angle) * spread };
      };

      simRef.current?.stop();
      nodesRef.current = nodes;
      edgesRef.current = edges;
      setCounts({ nodes: nodes.length, edges: edges.length, hidden });

      const simulation = forceSimulation(nodes)
        .force("charge", forceManyBody().strength(-90))
        .force(
          "link",
          forceLink<Node, Edge>(edges)
            .id((n) => n.id)
            .distance(46)
            .strength(0.4),
        )
        .force("center", forceCenter(0, 0).strength(0.04))
        .force("collide", forceCollide<Node>((n) => 6 + Math.min(n.degree, 12) * 0.7))
        .force("groupX", forceX<Node>((n) => anchor(n).x).strength(0.07))
        .force("groupY", forceY<Node>((n) => anchor(n).y).strength(0.07))
        .alphaDecay(0.035);
      userMovedRef.current = false;
      simulation.on("tick", () => {
        // Without this the camera sits at 1:1 around the origin and a vault of
        // twenty notes occupies a sixth of the workspace, the rest empty — the
        // map read as a rounding error.
        if (!userMovedRef.current) fitToNodes();
        schedulePaint();
      });
      simRef.current = simulation;
      // First paint must not wait for a frame that a hidden tab never gives.
      paintNow();
    })();
    return () => {
      cancelled = true;
      simRef.current?.stop();
    };
  }, [vault, vaults, allVaults, showReference, revision, schedulePaint, paintNow]);

  // Repaint when state that only affects appearance changes (selection, search
  // dimming, theme). Direct, so it works in a tab that is not compositing.
  useEffect(paintNow, [paintNow]);

  useEffect(() => {
    const element = wrapRef.current;
    if (!element) return;
    // The rail collapses and the detail column disappears at narrow widths, so
    // the canvas changes size without the window doing so.
    const observer = new ResizeObserver(() => paintNow());
    observer.observe(element);
    const onVisible = () => {
      if (!document.hidden) paintNow();
    };
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      observer.disconnect();
      document.removeEventListener("visibilitychange", onVisible);
    };
  }, [paintNow]);

  /** Bring a node to the middle — used when a search result is chosen. */
  const flyTo = useCallback(
    (key: string) => {
      const node = nodesRef.current.find((n) => n.key === key);
      if (!node?.x) return;
      const cam = cameraRef.current;
      const target = { x: -node.x * cam.k, y: -node.y! * cam.k };
      const from = { x: cam.x, y: cam.y };
      const start = performance.now();
      const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
      if (reduce) {
        cameraRef.current = { ...cam, ...target };
        schedulePaint();
        return;
      }
      const step = (now: number) => {
        const t = Math.min(1, (now - start) / 420);
        const ease = 1 - Math.pow(1 - t, 3);
        cameraRef.current = {
          ...cameraRef.current,
          x: from.x + (target.x - from.x) * ease,
          y: from.y + (target.y - from.y) * ease,
        };
        schedulePaint();
        if (t < 1) requestAnimationFrame(step);
      };
      requestAnimationFrame(step);
    },
    [schedulePaint],
  );

  useEffect(() => {
    if (selectedKey) flyTo(selectedKey);
  }, [selectedKey, flyTo]);

  // ---- pointer interaction ----
  const toWorld = (clientX: number, clientY: number) => {
    const rect = canvasRef.current!.getBoundingClientRect();
    const cam = cameraRef.current;
    return {
      x: (clientX - rect.left - rect.width / 2 - cam.x) / cam.k,
      y: (clientY - rect.top - rect.height / 2 - cam.y) / cam.k,
    };
  };

  const nodeAt = (clientX: number, clientY: number) => {
    const { x, y } = toWorld(clientX, clientY);
    let best: Node | null = null;
    let bestDist = Infinity;
    for (const node of nodesRef.current) {
      if (node.x === undefined) continue;
      const dx = node.x - x;
      const dy = node.y! - y;
      const dist = dx * dx + dy * dy;
      // Hit target is larger than the mark, so small nodes stay clickable.
      const reach = Math.pow(12 + Math.min(node.degree, 12) * 0.7, 2);
      if (dist < reach && dist < bestDist) {
        best = node;
        bestDist = dist;
      }
    }
    return best;
  };

  const onPointerDown = (e: React.PointerEvent<HTMLCanvasElement>) => {
    const node = nodeAt(e.clientX, e.clientY);
    if (node) {
      userMovedRef.current = true;
      dragRef.current = node;
      simRef.current?.alphaTarget(0.2).restart();
    } else {
      userMovedRef.current = true;
      panRef.current = { x: e.clientX - cameraRef.current.x, y: e.clientY - cameraRef.current.y };
    }
    (e.target as HTMLCanvasElement).setPointerCapture(e.pointerId);
  };

  const onPointerMove = (e: React.PointerEvent<HTMLCanvasElement>) => {
    if (dragRef.current) {
      const { x, y } = toWorld(e.clientX, e.clientY);
      dragRef.current.fx = x;
      dragRef.current.fy = y;
      schedulePaint();
      return;
    }
    if (panRef.current) {
      cameraRef.current = {
        ...cameraRef.current,
        x: e.clientX - panRef.current.x,
        y: e.clientY - panRef.current.y,
      };
      schedulePaint();
      return;
    }
    const node = nodeAt(e.clientX, e.clientY);
    if (node !== hoverRef.current) {
      hoverRef.current = node;
      const rect = canvasRef.current!.getBoundingClientRect();
      setHovered(
        node ? { node, x: e.clientX - rect.left, y: e.clientY - rect.top } : null,
      );
      schedulePaint();
    } else if (node && hovered) {
      const rect = canvasRef.current!.getBoundingClientRect();
      setHovered({ node, x: e.clientX - rect.left, y: e.clientY - rect.top });
    }
  };

  const endPointer = () => {
    if (dragRef.current) {
      dragRef.current.fx = null;
      dragRef.current.fy = null;
      dragRef.current = null;
      simRef.current?.alphaTarget(0);
    }
    panRef.current = null;
  };

  const onClick = (e: React.MouseEvent<HTMLCanvasElement>) => {
    const node = nodeAt(e.clientX, e.clientY);
    // A missing node is a link target, not a file: nothing to open.
    if (node && !node.missing) onSelect(node.vault, node.key);
  };

  const onWheel = (e: React.WheelEvent<HTMLCanvasElement>) => {
    e.preventDefault();
    const cam = cameraRef.current;
    const k = Math.min(4, Math.max(0.25, cam.k * (e.deltaY < 0 ? 1.12 : 0.9)));
    userMovedRef.current = true;
    cameraRef.current = { ...cam, k };
    schedulePaint();
  };

  const usedVaults = allVaults
    ? vaults.filter((v) => nodesRef.current.some((n) => n.vault === v))
    : [];

  return (
    <div className="graph-canvas-wrap" ref={wrapRef}>
      <canvas
        ref={canvasRef}
        className="graph-canvas"
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={endPointer}
        onPointerLeave={() => {
          endPointer();
          hoverRef.current = null;
          setHovered(null);
          schedulePaint();
        }}
        onClick={onClick}
        onWheel={onWheel}
        role="img"
        aria-label={t("graph.aria", { nodes: counts.nodes, edges: counts.edges })}
      />

      {hovered && (
        <div
          className="graph-tip"
          style={{ left: hovered.x, top: hovered.y }}
          aria-hidden
        >
          <b>{hovered.node.label}</b>
          <span className="path">{hovered.node.key}</span>
          <span className="graph-tip-meta">
            {hovered.node.missing
              ? t("graph.missing")
              : t("graph.links", { count: hovered.node.degree }) +
                (hovered.node.reference ? t("graph.readOnlySuffix") : "")}
          </span>
        </div>
      )}

      {/* Identity is never colour alone: the legend names every vault it uses. */}
      {usedVaults.length > 1 && (
        <div className="graph-legend">
          {usedVaults.map((name, i) => (
            <span key={name}>
              <span
                className="dot"
                style={{ background: VAULT_HUES[i] ?? NEUTRAL_HUE }}
                aria-hidden
              />
              {name}
            </span>
          ))}
        </div>
      )}

      {/* The legend is also the filter. There is no second place to look for a
          control that means exactly what the legend entry already says. */}
      <div className="graph-scale">
        <span className="dot solid" aria-hidden /> {t("graph.legend.own")}
        <button
          type="button"
          className={`legend-toggle ${showReference ? "on" : ""}`}
          onClick={() => setShowReference((on) => !on)}
          aria-pressed={showReference}
          title={t("graph.legend.referenceToggle")}
        >
          <span className="dot hollow" aria-hidden /> {t("graph.legend.reference")}
          {!showReference && counts.hidden > 0 && (
            <span className="legend-count">{counts.hidden}</span>
          )}
        </button>
        <span className="dot dashed" aria-hidden /> {t("graph.legend.missing")}
      </div>
    </div>
  );
}
