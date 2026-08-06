import {
  curveBounds,
  editableCurveToPathData,
  normalizedCurveNodes,
  pathDataToEditableCurve,
  pointAdd,
  pointLength,
  pointSubtract,
  setNodeType,
} from "./curvePath.js";

const SVG_NS = "http://www.w3.org/2000/svg";
const editorState = new WeakMap();
let activeSession = null;
let globalListenersAttached = false;

export function mountCurveEditor({
  mount,
  d,
  color,
  rotation,
  connectEndpoints,
  smoothSeamTangents,
  viewBounds = null,
  normalizePath = true,
  editable = false,
  editLabel = "",
  onChange,
}) {
  let state = editorState.get(mount);
  const previousCleanPath = state?.cleanPath;
  const sourcePath = canonicalOpenPathData(d.trim());
  const cleanPath = normalizePath ? normalizeCurvePathForEditor(d.trim()) : sourcePath;

  if (!state || previousCleanPath !== cleanPath) {
    state = {
      cleanPath,
      curve: pathDataToEditableCurve(cleanPath),
      viewBounds: null,
      activeRef: null,
      selectedRefs: new Set(),
      drag: null,
      modal: null,
      segmentTool: false,
      lastPointer: null,
      undoStack: [],
      redoStack: [],
      status: "",
    };
    editorState.set(mount, state);
  }

  state.onChange = onChange;
  state.connectEndpoints = Boolean(connectEndpoints);
  state.smoothSeamTangents = Boolean(smoothSeamTangents);
  state.rotation = rotation;
  state.color = color;
  state.editable = editable;
  state.editLabel = editLabel;
  state.fixedViewBounds = normalizeViewBounds(viewBounds);

  if (editable && sourcePath !== cleanPath) {
    queueMicrotask(() => {
      if (activeSession?.state !== state) return;
      state.onChange?.(cleanPath);
    });
  }

  if (editable) {
    activeSession = { mount, state };
    ensureGlobalListeners();
  } else if (activeSession?.mount === mount) {
    activeSession = null;
  }

  renderEditor(mount, state);
}

export function clearCurveEditor(mount) {
  editorState.delete(mount);
  if (activeSession?.mount === mount) {
    activeSession = null;
  }
}

function renderEditor(mount, state) {
  mount.innerHTML = "";
  mount.classList.add("curve-editor-mount");
  mount.classList.toggle("curve-editor-active", state.editable);
  mount.classList.toggle("curve-editor-inactive", !state.editable);

  normalizeStateCurve(state);
  if (state.fixedViewBounds) {
    state.viewBounds = { ...state.fixedViewBounds };
  } else if (!state.viewBounds) {
    state.viewBounds = editorViewBounds(state.curve);
  }

  const curve = { nodes: state.curve.nodes.map(cloneNode) };
  const pathData = editableCurveToPathData(curve, { connectEndpoints: false });
  const bounds = state.viewBounds;
  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("viewBox", `${bounds.minX} ${bounds.minY} ${bounds.width} ${bounds.height}`);
  svg.setAttribute("class", "curve-editor");
  svg.setAttribute("tabindex", "0");
  svg.dataset.connectState = state.connectEndpoints
    ? state.smoothSeamTangents
      ? "render-smooth-seam"
      : "render-corner-seam"
    : "render-open";

  const path = svgEl("path", {
    class: "curve-editor-source",
    d: pathData,
    fill: "none",
    stroke: state.color,
  });
  appendChainedCurvePreview(svg, curve, pathData, state, bounds);
  svg.append(path);
  appendAdvanceGuide(svg, bounds, state);

  if (state.editable) {
    appendSegmentHitTargets(svg, curve);

    curve.nodes.forEach((node, index) => {
      appendHandle(svg, state, bounds, index, "in", node);
      appendHandle(svg, state, bounds, index, "out", node);
    });

    curve.nodes.forEach((node, index) => {
      svg.append(svgEl("circle", {
        class: `curve-node ${nodeClass(node)}${selected(state, nodeRef(index)) ? " selected" : ""}${isEndpoint(curve.nodes, index) ? " endpoint" : ""}`,
        cx: node.position.x,
        cy: node.position.y,
        r: controlSize(bounds) * 1.12,
        "data-kind": "node",
        "data-index": String(index),
      }));
    });
  }

  const shell = document.createElement("div");
  shell.className = "curve-editor-shell";
  shell.append(svg);
  shell.append(renderTools(state));
  mount.append(shell);

  if (state.editable) {
    svg.addEventListener("pointerdown", (event) => handlePointerDown(event, svg, state, mount));
    svg.addEventListener("pointermove", (event) => {
      state.lastPointer = svgPoint(svg, event);
      if (state.segmentTool) renderEditor(mount, state);
    });
    svg.addEventListener("dblclick", (event) => handleDoubleClick(event, svg, state, mount));
    svg.focus({ preventScroll: true });
  }
}

function appendAdvanceGuide(svg, bounds, state) {
  const radians = (Number(state.rotation) || 0) * Math.PI / 180;
  const length = Math.max(bounds.width, bounds.height) * 0.24;
  const center = {
    x: bounds.minX + bounds.width - length * 0.8,
    y: bounds.minY + length * 0.8,
  };
  const end = {
    x: center.x + Math.cos(radians) * length,
    y: center.y + Math.sin(radians) * length,
  };
  svg.append(svgEl("line", {
    class: "curve-advance-guide",
    x1: center.x,
    y1: center.y,
    x2: end.x,
    y2: end.y,
  }));
  svg.append(svgEl("circle", {
    class: "curve-advance-guide-dot",
    cx: end.x,
    cy: end.y,
    r: controlSize(bounds) * 0.65,
  }));
}

function appendChainedCurvePreview(svg, curve, pathData, state, bounds) {
  const advance = curveAdvanceVector(curve);
  if (pointLength(advance) <= 0.0001) return;

  const clipId = `curve-chain-preview-${Math.random().toString(36).slice(2)}`;
  const defs = svgEl("defs", {});
  const clipPath = svgEl("clipPath", { id: clipId });
  clipPath.append(svgEl("rect", {
    x: bounds.minX,
    y: bounds.minY,
    width: bounds.width,
    height: bounds.height,
  }));
  defs.append(clipPath);
  svg.append(defs);

  const group = svgEl("g", {
    class: "curve-editor-chain-preview",
    "clip-path": `url(#${clipId})`,
  });
  group.append(svgEl("path", {
    d: pathData,
    fill: "none",
    stroke: state.color,
    transform: `translate(${-advance.x} ${-advance.y})`,
  }));
  group.append(svgEl("path", {
    d: pathData,
    fill: "none",
    stroke: state.color,
    transform: `translate(${advance.x} ${advance.y})`,
  }));
  svg.append(group);
}

function editorViewBounds(curve) {
  return paddedBounds(curveBounds(curve), 0.55);
}

function curveAdvanceVector(curve) {
  const nodes = curve.nodes ?? [];
  const start = nodes[0]?.position;
  const end = nodes.at(-1)?.position;
  if (!start || !end) return { x: 1, y: 0 };

  const endpointAdvance = pointSubtract(end, start);
  if (pointLength(endpointAdvance) > 0.0001) {
    return endpointAdvance;
  }

  const bounds = curveBounds(curve);
  return {
    x: Math.max(bounds.width, 1),
    y: 0,
  };
}

function appendSegmentHitTargets(svg, curve) {
  for (let index = 0; index < curve.nodes.length - 1; index += 1) {
    const start = curve.nodes[index];
    const end = curve.nodes[index + 1];
    const d = segmentPathData(start, end);
    svg.append(svgEl("path", {
      class: `curve-segment-hit${selected(stateFromSvg(svg), segmentRef(index)) ? " selected" : ""}`,
      d,
      fill: "none",
      stroke: "transparent",
      "stroke-width": "14",
      "data-kind": "segment",
      "data-index": String(index),
    }));
  }
}

function stateFromSvg(svg) {
  return activeSession?.mount?.contains(svg) ? activeSession.state : { selectedRefs: new Set() };
}

function appendHandle(svg, state, bounds, index, side, node) {
  const handle = side === "in" ? node.handleIn : node.handleOut;
  const endpoint = pointAdd(node.position, handle);

  svg.append(svgEl("line", {
    class: "curve-handle-line",
    x1: node.position.x,
    y1: node.position.y,
    x2: endpoint.x,
    y2: endpoint.y,
  }));
  svg.append(svgEl("circle", {
    class: `curve-handle ${selected(state, handleRef(index, side)) ? "selected" : ""}${pointLength(handle) <= 0.0001 ? " zero" : ""}`,
    cx: endpoint.x,
    cy: endpoint.y,
    r: controlSize(bounds) * 0.9,
    "data-kind": "handle",
    "data-index": String(index),
    "data-side": side,
  }));
}

function renderTools(state) {
  const tools = document.createElement("div");
  tools.className = "curve-editor-tools";

  const activeNode = state.activeRef?.kind === "node"
    ? state.curve.nodes[state.activeRef.index]
    : null;
  const nodeType = activeNode ? displayNodeType(activeNode) : "";

  tools.innerHTML = `
    <span class="edit-state">${state.editable ? state.editLabel || "Editing curve" : "Preview only"}</span>
    <span class="seam-state">${seamLabel(state)}</span>
    <span class="curve-editor-shortcuts">${shortcutLabel(state)}</span>
    <label class="curve-node-type-label">
      Active node type
      <select class="curve-node-type" ${activeNode && state.editable ? "" : "disabled"}>
        <option value="">Select node</option>
        <option value="free">Free/corner</option>
        <option value="vector">Vector</option>
        <option value="smooth">Smooth</option>
        <option value="symmetrical">Symmetrical</option>
      </select>
    </label>
  `;

  const select = tools.querySelector(".curve-node-type");
  select.value = nodeType;
  select.addEventListener("change", () => {
    if (!state.activeRef || state.activeRef.kind !== "node" || !select.value) return;
    state.curve = setNodeType(state.curve, state.activeRef.index, select.value);
    commitCleanPath(state);
    renderEditor(activeSession.mount, state);
  });

  return tools;
}

function handlePointerDown(event, svg, state, mount) {
  if (!state.editable) return;
  svg.focus({ preventScroll: true });
  const pointer = svgPoint(svg, event);
  state.lastPointer = pointer;

  if (state.modal) {
    if (event.button === 0) {
      confirmModal(state);
      event.preventDefault();
      renderEditor(mount, state);
    }
    return;
  }

  if (state.segmentTool && event.button === 0) {
    addNodeAtPointer(state, pointer);
    event.preventDefault();
    commitCleanPath(state);
    renderEditor(mount, state);
    return;
  }

  const target = event.target;
  const kind = target.dataset?.kind;
  if (!kind) {
    clearSelection(state);
    renderEditor(mount, state);
    return;
  }

  event.preventDefault();
  const ref = refFromTarget(target);
  if (!ref) return;

  const alreadySelected = selected(state, ref);
  selectRef(state, ref, event.shiftKey);
  const dragRefs = alreadySelected && !event.shiftKey
    ? selectedRefsForDrag(state)
    : [ref];

  state.drag = {
    refs: dragRefs,
    startPointer: pointer,
    startCurve: cloneCurve(state.curve),
  };

  target.setPointerCapture?.(event.pointerId);

  const move = (moveEvent) => {
    if (!state.drag) return;
    const liveSvg = mount.querySelector("svg.curve-editor") ?? svg;
    state.lastPointer = svgPoint(liveSvg, moveEvent);
    applyDrag(state, state.lastPointer, moveEvent);
    renderEditor(mount, state);
  };
  const up = () => {
    state.drag = null;
    commitCleanPath(state);
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", up);
  };

  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", up);
  renderEditor(mount, state);
}

function handleDoubleClick(event, svg, state, mount) {
  if (!state.editable) return;
  const pointer = svgPoint(svg, event);
  const kind = event.target.dataset?.kind;

  if (kind === "node") {
    const index = Number(event.target.dataset.index);
    state.curve = setNodeType(state.curve, index, nextNodeType(state.curve.nodes[index]));
    selectRef(state, nodeRef(index), false);
  } else if (kind === "segment") {
    insertNodeOnSegment(state, Number(event.target.dataset.index), pointer);
  } else {
    addNodeAtPointer(state, pointer);
  }

  event.preventDefault();
  commitCleanPath(state);
  renderEditor(mount, state);
}

function applyDrag(state, pointer, event = null) {
  const drag = state.drag;
  if (!drag) return;

  state.curve = cloneCurve(drag.startCurve);
  let delta = pointSubtract(pointer, drag.startPointer);
  if (event?.shiftKey) {
    delta = constrainDelta(delta);
  }

  for (const ref of drag.refs) {
    if (ref.kind === "node") {
      const node = state.curve.nodes[ref.index];
      if (node) node.position = pointAdd(node.position, delta);
      continue;
    }

    if (ref.kind === "handle") {
      const node = state.curve.nodes[ref.index];
      if (!node) continue;
      const startNode = drag.startCurve.nodes[ref.index];
      const base = ref.side === "in" ? startNode.handleIn : startNode.handleOut;
      const handle = pointAdd(base, delta);
      setHandle(state, ref.index, ref.side, handle);
    }
  }
}

function ensureGlobalListeners() {
  if (globalListenersAttached) return;
  globalListenersAttached = true;
  window.addEventListener("keydown", handleKeyDown, { capture: true });
  window.addEventListener("pointermove", handleGlobalPointerMove, { capture: true });
}

function handleGlobalPointerMove(event) {
  const session = activeSession;
  if (!session?.state?.editable || !session.state.modal) return;
  const svg = session.mount.querySelector("svg.curve-editor");
  if (!svg) return;
  const pointer = svgPoint(svg, event);
  session.state.lastPointer = pointer;
  updateModal(session.state, pointer, event);
  renderEditor(session.mount, session.state);
}

function handleKeyDown(event) {
  const session = activeSession;
  const state = session?.state;
  const mount = session?.mount;
  if (!state?.editable || !mount) return;
  if (event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement || event.target instanceof HTMLSelectElement) {
    return;
  }

  const key = event.key.toLowerCase();

  if ((event.ctrlKey || event.metaKey) && key === "z" && event.shiftKey) {
    redo(state);
    event.preventDefault();
    renderEditor(mount, state);
    return;
  }

  if ((event.ctrlKey || event.metaKey) && key === "z") {
    undo(state);
    event.preventDefault();
    renderEditor(mount, state);
    return;
  }

  if (state.modal) {
    if (event.key === "Escape") {
      cancelModal(state);
      event.preventDefault();
    } else if (event.key === "Enter") {
      confirmModal(state);
      event.preventDefault();
    } else if (key === "x") {
      state.modal.axis = state.modal.axis === "x" ? null : "x";
      event.preventDefault();
    } else if (key === "y") {
      state.modal.axis = state.modal.axis === "y" ? null : "y";
      event.preventDefault();
    }
    renderEditor(mount, state);
    return;
  }

  if (event.key === "Escape") {
    if (state.segmentTool) {
      state.segmentTool = false;
      state.status = "Segment mode cancelled";
    } else {
      clearSelection(state);
    }
    event.preventDefault();
    renderEditor(mount, state);
    return;
  }

  if (event.key === "Enter" && state.segmentTool) {
    state.segmentTool = false;
    state.status = "Segment mode complete";
    event.preventDefault();
    renderEditor(mount, state);
    return;
  }

  if (event.key === "Tab") {
    state.segmentTool = !state.segmentTool;
    state.status = state.segmentTool ? "Segment mode: click to append nodes" : "Edit mode";
    event.preventDefault();
    renderEditor(mount, state);
    return;
  }

  if (event.altKey && key === "a") {
    clearSelection(state);
    event.preventDefault();
    renderEditor(mount, state);
    return;
  }

  if (key === "a" && !event.ctrlKey && !event.metaKey) {
    selectAllNodes(state);
    event.preventDefault();
    renderEditor(mount, state);
    return;
  }

  if (key === "p") {
    state.segmentTool = !state.segmentTool;
    state.status = state.segmentTool ? "Segment mode: click to append nodes; Enter finishes" : "Edit mode";
    event.preventDefault();
    renderEditor(mount, state);
    return;
  }

  if (key === "v") {
    openNodeTypePicker(mount, state);
    event.preventDefault();
    renderEditor(mount, state);
    return;
  }

  const shortcutNodeType = nodeTypeForShortcutKey(key);
  if (shortcutNodeType && !event.ctrlKey && !event.metaKey && !event.altKey) {
    setSelectedNodeType(state, shortcutNodeType);
    event.preventDefault();
    commitCleanPath(state);
    renderEditor(mount, state);
    return;
  }

  if (key === "e") {
    extrudeEndpoint(state);
    event.preventDefault();
    commitCleanPath(state);
    renderEditor(mount, state);
    return;
  }

  if (key === "m") {
    mergeSelectedNodes(state);
    event.preventDefault();
    commitCleanPath(state);
    renderEditor(mount, state);
    return;
  }

  if (key === "f") {
    selectChainEndpoints(state);
    event.preventDefault();
    renderEditor(mount, state);
    return;
  }

  if (key === "b") {
    state.status = "Break is disabled for chained single-curve editing";
    event.preventDefault();
    renderEditor(mount, state);
    return;
  }

  if (key === "g" || key === "s" || key === "r") {
    startModal(state, key === "g" ? "move" : key === "s" ? "scale" : "rotate");
    event.preventDefault();
    renderEditor(mount, state);
    return;
  }

  if (event.key === ".") {
    state.viewBounds = state.fixedViewBounds ? { ...state.fixedViewBounds } : editorViewBounds(state.curve);
    state.status = state.fixedViewBounds ? "Framed artboard-scale curve" : "Framed editable source curve";
    event.preventDefault();
    renderEditor(mount, state);
    return;
  }

  if (event.key === "Delete" || event.key === "Backspace" || key === "x") {
    deleteSelection(state);
    event.preventDefault();
    commitCleanPath(state);
    renderEditor(mount, state);
  }
}

function startModal(state, kind) {
  const refs = selectedRefsForDrag(state).filter((ref) => ref.kind === "node" || ref.kind === "handle");
  if (refs.length === 0) {
    state.status = "Select nodes or handles first";
    return;
  }
  const pointer = state.lastPointer ?? curveCenter(state.curve);
  state.modal = {
    kind,
    refs,
    startCurve: cloneCurve(state.curve),
    startPointer: pointer,
    pivot: selectionPivot(state, refs),
    axis: null,
  };
  state.status = `${title(kind)} transform: move pointer; X/Y constrain; Enter confirms; Esc cancels`;
}

function updateModal(state, pointer, event = null) {
  const modal = state.modal;
  if (!modal) return;
  state.curve = cloneCurve(modal.startCurve);

  let delta = pointSubtract(pointer, modal.startPointer);
  if (modal.axis === "x") delta = { x: delta.x, y: 0 };
  if (modal.axis === "y") delta = { x: 0, y: delta.y };
  if (event?.shiftKey) delta = { x: delta.x * 0.2, y: delta.y * 0.2 };

  if (modal.kind === "move") {
    transformMove(state, modal.refs, delta);
  } else if (modal.kind === "scale") {
    const factor = Math.max(0.02, 1 + delta.x / Math.max(1, curveBounds(modal.startCurve).width));
    transformScale(state, modal.refs, modal.pivot, factor);
  } else if (modal.kind === "rotate") {
    transformRotate(state, modal.refs, modal.pivot, delta.x * 0.8);
  }
}

function confirmModal(state) {
  if (!state.modal) return;
  state.modal = null;
  commitCleanPath(state);
  state.status = "Transform committed";
}

function cancelModal(state) {
  if (!state.modal) return;
  state.curve = cloneCurve(state.modal.startCurve);
  state.modal = null;
  state.status = "Transform cancelled";
}

function transformMove(state, refs, delta) {
  for (const ref of refs) {
    if (ref.kind === "node") {
      const node = state.curve.nodes[ref.index];
      if (node) node.position = pointAdd(node.position, delta);
    } else if (ref.kind === "handle") {
      const node = state.curve.nodes[ref.index];
      if (!node) continue;
      setHandle(state, ref.index, ref.side, pointAdd(ref.side === "in" ? node.handleIn : node.handleOut, delta));
    }
  }
}

function transformScale(state, refs, pivot, factor) {
  for (const ref of refs) {
    const node = state.curve.nodes[ref.index];
    if (!node) continue;
    if (ref.kind === "node") {
      const original = node.position;
      node.position = {
        x: pivot.x + (original.x - pivot.x) * factor,
        y: pivot.y + (original.y - pivot.y) * factor,
      };
      node.handleIn = scalePoint(node.handleIn, factor);
      node.handleOut = scalePoint(node.handleOut, factor);
    } else if (ref.kind === "handle") {
      const handle = ref.side === "in" ? node.handleIn : node.handleOut;
      setHandle(state, ref.index, ref.side, scalePoint(handle, factor));
    }
  }
}

function transformRotate(state, refs, pivot, degrees) {
  const radians = degrees * Math.PI / 180;
  for (const ref of refs) {
    const node = state.curve.nodes[ref.index];
    if (!node) continue;
    if (ref.kind === "node") {
      node.position = rotatePoint(node.position, pivot, radians);
      node.handleIn = rotateVector(node.handleIn, radians);
      node.handleOut = rotateVector(node.handleOut, radians);
    } else if (ref.kind === "handle") {
      const handle = ref.side === "in" ? node.handleIn : node.handleOut;
      setHandle(state, ref.index, ref.side, rotateVector(handle, radians));
    }
  }
}

function addNodeAtPointer(state, pointer) {
  const activeNodeIndex = state.activeRef?.kind === "node" ? state.activeRef.index : state.curve.nodes.length - 1;
  addNodeAfter(state, activeNodeIndex, pointer);
}

function insertNodeOnSegment(state, index, pointer) {
  addNodeAfter(state, index, pointer);
}

function addNodeAfter(state, index, position) {
  normalizeStateCurve(state);
  const nodes = state.curve.nodes;
  if (nodes.length === 0 || index < 0) {
    nodes.unshift(createNode(position));
    selectRef(state, nodeRef(0), false);
    return;
  }

  const previous = nodes[index];
  const next = nodes[index + 1];
  const node = createNode(position);

  if (previous) {
    previous.handleOut = scalePoint(pointSubtract(position, previous.position), 1 / 3);
    previous.handleOutBehavior = "vector";
    node.handleIn = scalePoint(pointSubtract(previous.position, position), 1 / 3);
    node.handleInBehavior = "vector";
  }

  if (next) {
    node.handleOut = scalePoint(pointSubtract(next.position, position), 1 / 3);
    node.handleOutBehavior = "vector";
    next.handleIn = scalePoint(pointSubtract(position, next.position), 1 / 3);
    next.handleInBehavior = "vector";
  }

  nodes.splice(index + 1, 0, node);
  selectRef(state, nodeRef(index + 1), false);
  state.status = `Added node ${index + 2}`;
}

function extrudeEndpoint(state) {
  normalizeStateCurve(state);
  const nodes = state.curve.nodes;
  if (nodes.length < 1) return;

  const selectedNodes = selectedNodeIndexes(state);
  const firstSelected = selectedNodes.includes(0);
  const lastSelected = selectedNodes.includes(nodes.length - 1);

  if (firstSelected && !lastSelected) {
    const first = nodes[0];
    const second = nodes[1];
    const delta = second ? pointSubtract(first.position, second.position) : { x: -0.5, y: 0 };
    const position = pointAdd(first.position, delta);
    const node = createNode(position);
    node.handleOut = scalePoint(pointSubtract(first.position, position), 1 / 3);
    node.handleOutBehavior = "vector";
    first.handleIn = scalePoint(pointSubtract(position, first.position), 1 / 3);
    first.handleInBehavior = "vector";
    nodes.unshift(node);
    selectRef(state, nodeRef(0), false);
    state.status = "Extruded start endpoint";
    return;
  }

  const last = nodes.at(-1);
  const previous = nodes.at(-2);
  const delta = previous ? pointSubtract(last.position, previous.position) : { x: 0.5, y: 0 };
  const position = pointAdd(last.position, delta);
  addNodeAfter(state, nodes.length - 1, position);
  state.status = "Extruded end endpoint";
}

function deleteSelection(state) {
  const handles = selectedHandleRefs(state);
  if (handles.length > 0 && selectedNodeIndexes(state).length === 0) {
    for (const ref of handles) {
      setHandle(state, ref.index, ref.side, { x: 0, y: 0 });
    }
    clearSelection(state);
    state.status = `Collapsed ${handles.length} handle${handles.length === 1 ? "" : "s"}`;
    return;
  }

  const nodeIndexes = selectedNodeIndexes(state).sort((a, b) => b - a);
  for (const index of nodeIndexes) {
    if (state.curve.nodes.length <= 2) break;
    state.curve.nodes.splice(index, 1);
  }
  clearSelection(state);
  state.status = `Deleted ${nodeIndexes.length} node${nodeIndexes.length === 1 ? "" : "s"}`;
}

function mergeSelectedNodes(state) {
  const indexes = selectedNodeIndexes(state).sort((a, b) => a - b);
  if (indexes.length < 2) {
    state.status = "Select at least two nodes to merge";
    return;
  }

  const average = indexes.reduce(
    (acc, index) => pointAdd(acc, state.curve.nodes[index].position),
    { x: 0, y: 0 },
  );
  average.x /= indexes.length;
  average.y /= indexes.length;

  const keepIndex = indexes[0];
  state.curve.nodes[keepIndex].position = average;
  for (const index of indexes.slice(1).sort((a, b) => b - a)) {
    if (state.curve.nodes.length <= 2) break;
    state.curve.nodes.splice(index, 1);
  }
  selectRef(state, nodeRef(keepIndex), false);
  state.status = `Merged ${indexes.length} nodes`;
}

function cycleActiveNodeType(state) {
  const index = state.activeRef?.kind === "node"
    ? state.activeRef.index
    : selectedNodeIndexes(state)[0];
  if (!Number.isFinite(index)) {
    state.status = "Select a node first";
    return;
  }
  state.curve = setNodeType(state.curve, index, nextNodeType(state.curve.nodes[index]));
  selectRef(state, nodeRef(index), false);
}

function setSelectedNodeType(state, type) {
  const indexes = selectedNodeIndexes(state);
  const activeIndex = state.activeRef?.kind === "node" ? state.activeRef.index : null;
  const targetIndexes = indexes.length > 0
    ? indexes
    : Number.isFinite(activeIndex)
      ? [activeIndex]
      : [];

  if (targetIndexes.length === 0) {
    state.status = "Select a node first";
    return;
  }

  for (const index of targetIndexes) {
    state.curve = setNodeType(state.curve, index, type);
  }
  state.activeRef = nodeRef(targetIndexes.at(-1));
  state.status = `Set ${targetIndexes.length} node${targetIndexes.length === 1 ? "" : "s"} to ${displayTypeName(type)}`;
}

function openNodeTypePicker(mount, state) {
  const activeIndex = state.activeRef?.kind === "node" ? state.activeRef.index : selectedNodeIndexes(state)[0];
  if (!Number.isFinite(activeIndex)) {
    state.status = "Select a node first";
    return;
  }

  state.activeRef = nodeRef(activeIndex);
  queueMicrotask(() => {
    const select = mount.querySelector(".curve-node-type");
    select?.focus({ preventScroll: true });
    try {
      select?.showPicker?.();
    } catch {
      // Browser support and user-activation rules vary; focus still exposes the menu control.
    }
  });
  state.status = "Node type menu: Free, Vector, Smooth, Symmetrical";
}

function selectChainEndpoints(state) {
  const lastIndex = state.curve.nodes.length - 1;
  if (lastIndex <= 0) {
    state.status = "Need at least two nodes for a chain seam";
    return;
  }

  state.selectedRefs = new Set([refKey(nodeRef(0)), refKey(nodeRef(lastIndex))]);
  state.activeRef = nodeRef(lastIndex);
  state.status = "Selected chained seam endpoints";
}

function setHandle(state, index, side, handle) {
  const node = state.curve.nodes[index];
  if (!node) return;
  if (side === "in") {
    node.handleIn = handle;
    node.handleInBehavior = "free";
  } else {
    node.handleOut = handle;
    node.handleOutBehavior = "free";
  }

  if (node.pairConstraint === "smooth" || node.pairConstraint === "symmetrical") {
    state.curve = setNodeType(state.curve, index, node.pairConstraint);
  }
}

function commitCleanPath(state) {
  normalizeStateCurve(state);
  const nextPath = editableCurveToPathData(state.curve, { connectEndpoints: false });
  if (nextPath === state.cleanPath) return;

  state.undoStack.push(state.cleanPath);
  state.redoStack = [];
  state.cleanPath = nextPath;
  state.onChange?.(nextPath);
}

function undo(state) {
  const previous = state.undoStack.pop();
  if (!previous) {
    state.status = "Nothing to undo";
    return;
  }
  state.redoStack.push(state.cleanPath);
  state.cleanPath = previous;
  state.curve = pathDataToEditableCurve(previous);
  clearSelection(state);
  state.onChange?.(previous);
  state.status = "Undo";
}

function redo(state) {
  const next = state.redoStack.pop();
  if (!next) {
    state.status = "Nothing to redo";
    return;
  }
  state.undoStack.push(state.cleanPath);
  state.cleanPath = next;
  state.curve = pathDataToEditableCurve(next);
  clearSelection(state);
  state.onChange?.(next);
  state.status = "Redo";
}

function normalizeStateCurve(state) {
  state.curve = { nodes: normalizedCurveNodes(state.curve, { connectEndpoints: false }) };
  if (state.curve.nodes.length === 1) {
    state.curve.nodes.push(createNode(pointAdd(state.curve.nodes[0].position, { x: 1, y: 0 })));
  }
}

function canonicalOpenPathData(pathData) {
  return editableCurveToPathData(pathDataToEditableCurve(pathData), { connectEndpoints: false });
}

export function normalizeCurvePathForEditor(pathData) {
  const curve = pathDataToEditableCurve(pathData);
  return editableCurveToPathData(fitCurveToEditorSpace(curve), { connectEndpoints: false });
}

function fitCurveToEditorSpace(curve) {
  const source = { nodes: normalizedCurveNodes(curve, { connectEndpoints: false }) };
  if (source.nodes.length === 0) {
    return {
      nodes: [
        createNode({ x: -0.5, y: 0 }),
        createNode({ x: 0.5, y: 0 }),
      ],
    };
  }

  const bounds = curveBounds(source);
  const maxDimension = Math.max(bounds.width, bounds.height);
  if (maxDimension <= 0.0001) {
    return source;
  }

  const scale = 1 / maxDimension;
  const center = {
    x: bounds.minX + bounds.width / 2,
    y: bounds.minY + bounds.height / 2,
  };

  return {
    nodes: source.nodes.map((node) => ({
      ...cloneNode(node),
      position: {
        x: (node.position.x - center.x) * scale,
        y: (node.position.y - center.y) * scale,
      },
      handleIn: scalePoint(node.handleIn, scale),
      handleOut: scalePoint(node.handleOut, scale),
    })),
  };
}

function selectRef(state, ref, additive) {
  if (!additive) {
    state.selectedRefs = new Set([refKey(ref)]);
  } else {
    const key = refKey(ref);
    if (state.selectedRefs.has(key)) {
      state.selectedRefs.delete(key);
    } else {
      state.selectedRefs.add(key);
    }
  }
  state.activeRef = ref;
}

function clearSelection(state) {
  state.selectedRefs = new Set();
  state.activeRef = null;
}

function selectAllNodes(state) {
  state.selectedRefs = new Set(state.curve.nodes.map((_, index) => refKey(nodeRef(index))));
  state.activeRef = nodeRef(Math.max(0, state.curve.nodes.length - 1));
  state.status = `Selected ${state.curve.nodes.length} nodes`;
}

function selected(state, ref) {
  return state.selectedRefs?.has(refKey(ref));
}

function selectedRefsForDrag(state) {
  const refs = [...state.selectedRefs].map(parseRefKey).filter(Boolean);
  return refs.length > 0 ? refs : state.activeRef ? [state.activeRef] : [];
}

function selectedNodeIndexes(state) {
  return [...state.selectedRefs]
    .map(parseRefKey)
    .filter((ref) => ref?.kind === "node")
    .map((ref) => ref.index)
    .filter((index) => index >= 0 && index < state.curve.nodes.length);
}

function selectedHandleRefs(state) {
  return [...state.selectedRefs]
    .map(parseRefKey)
    .filter((ref) => ref?.kind === "handle")
    .filter((ref) => ref.index >= 0 && ref.index < state.curve.nodes.length);
}

function refFromTarget(target) {
  const kind = target.dataset?.kind;
  if (kind === "node") return nodeRef(Number(target.dataset.index));
  if (kind === "handle") return handleRef(Number(target.dataset.index), target.dataset.side);
  if (kind === "segment") return segmentRef(Number(target.dataset.index));
  return null;
}

function nodeRef(index) {
  return { kind: "node", index };
}

function handleRef(index, side) {
  return { kind: "handle", index, side };
}

function segmentRef(index) {
  return { kind: "segment", index };
}

function refKey(ref) {
  return `${ref.kind}:${ref.index}:${ref.side ?? ""}`;
}

function parseRefKey(key) {
  const [kind, index, side] = key.split(":");
  if (!kind || !Number.isFinite(Number(index))) return null;
  if (kind === "handle") return handleRef(Number(index), side);
  if (kind === "segment") return segmentRef(Number(index));
  return nodeRef(Number(index));
}

function createNode(position) {
  return {
    position: { x: finite(position.x), y: finite(position.y) },
    handleIn: { x: 0, y: 0 },
    handleOut: { x: 0, y: 0 },
    handleInBehavior: "free",
    handleOutBehavior: "free",
    pairConstraint: "independent",
  };
}

function cloneCurve(curve) {
  return { nodes: (curve.nodes ?? []).map(cloneNode) };
}

function cloneNode(node) {
  return {
    position: { x: finite(node.position?.x), y: finite(node.position?.y) },
    handleIn: { x: finite(node.handleIn?.x), y: finite(node.handleIn?.y) },
    handleOut: { x: finite(node.handleOut?.x), y: finite(node.handleOut?.y) },
    handleInBehavior: node.handleInBehavior ?? "free",
    handleOutBehavior: node.handleOutBehavior ?? "free",
    pairConstraint: node.pairConstraint ?? "independent",
  };
}

function segmentPathData(start, end) {
  if (isLineLike(start, end)) {
    return `M ${formatPoint(start.position)} L ${formatPoint(end.position)}`;
  }
  return `M ${formatPoint(start.position)} C ${formatPoint(pointAdd(start.position, start.handleOut))} ${formatPoint(pointAdd(end.position, end.handleIn))} ${formatPoint(end.position)}`;
}

function isLineLike(start, end) {
  return start.handleOutBehavior === "vector" && end.handleInBehavior === "vector";
}

function selectionPivot(state, refs) {
  const points = refs.flatMap((ref) => {
    const node = state.curve.nodes[ref.index];
    if (!node) return [];
    if (ref.kind === "handle") {
      return [pointAdd(node.position, ref.side === "in" ? node.handleIn : node.handleOut)];
    }
    return [node.position];
  });

  if (points.length === 0) return curveCenter(state.curve);
  return {
    x: points.reduce((sum, point) => sum + point.x, 0) / points.length,
    y: points.reduce((sum, point) => sum + point.y, 0) / points.length,
  };
}

function curveCenter(curve) {
  const bounds = curveBounds(curve);
  return { x: bounds.minX + bounds.width / 2, y: bounds.minY + bounds.height / 2 };
}

function svgPoint(svg, event) {
  const point = svg.createSVGPoint();
  point.x = event.clientX;
  point.y = event.clientY;
  const matrix = svg.getScreenCTM();
  if (!matrix) return { x: 0, y: 0 };
  const transformed = point.matrixTransform(matrix.inverse());
  return { x: transformed.x, y: transformed.y };
}

function paddedBounds(bounds, paddingScale = 0.22) {
  const pad = Math.max(bounds.width, bounds.height, 1) * paddingScale;
  return {
    minX: bounds.minX - pad,
    minY: bounds.minY - pad,
    maxX: bounds.maxX + pad,
    maxY: bounds.maxY + pad,
    width: bounds.width + pad * 2,
    height: bounds.height + pad * 2,
    centerX: bounds.minX + bounds.width / 2,
    centerY: bounds.minY + bounds.height / 2,
  };
}

function controlSize(bounds) {
  return Math.max(bounds.width, bounds.height) * 0.008;
}

function normalizeViewBounds(bounds) {
  if (!bounds) return null;
  const minX = finite(bounds.minX ?? bounds.x);
  const minY = finite(bounds.minY ?? bounds.y);
  const width = Math.max(0.001, finite(bounds.width));
  const height = Math.max(0.001, finite(bounds.height));
  return {
    minX,
    minY,
    maxX: minX + width,
    maxY: minY + height,
    width,
    height,
  };
}

function svgEl(name, attrs) {
  const element = document.createElementNS(SVG_NS, name);
  for (const [key, value] of Object.entries(attrs)) {
    element.setAttribute(key, value);
  }
  return element;
}

function seamLabel(state) {
  const renderSeam = !state.connectEndpoints
    ? "Render seam: open"
    : state.smoothSeamTangents
      ? "Render seam: connected/smooth"
      : "Render seam: connected/corner";
  return `${renderSeam}; editor path remains open`;
}

function shortcutLabel(state) {
  if (state.modal) return `${title(state.modal.kind)} active · X/Y constrain · Enter commit · Esc cancel`;
  if (state.segmentTool) return "Segment mode · click adds nodes · Enter finish · Esc cancel";
  return "A select all · Alt+A clear · P add segments · E extrude · V node type · 1-4 set type · F seam endpoints · G/S/R transform · Del/X delete";
}

function displayNodeType(node) {
  if (!node) return "";
  if (node.handleInBehavior === "vector" || node.handleOutBehavior === "vector") return "vector";
  if (node.pairConstraint === "smooth") return "smooth";
  if (node.pairConstraint === "symmetrical") return "symmetrical";
  return "free";
}

function nextNodeType(node) {
  const order = ["free", "vector", "smooth", "symmetrical"];
  const current = displayNodeType(node);
  return order[(order.indexOf(current) + 1) % order.length];
}

function nodeTypeForShortcutKey(key) {
  if (key === "1") return "free";
  if (key === "2") return "vector";
  if (key === "3") return "smooth";
  if (key === "4") return "symmetrical";
  return "";
}

function displayTypeName(type) {
  if (type === "free") return "Free";
  if (type === "vector") return "Vector";
  if (type === "smooth") return "Smooth";
  if (type === "symmetrical") return "Symmetrical";
  return type;
}

function nodeClass(node) {
  return ` ${displayNodeType(node)}`;
}

function isEndpoint(nodes, index) {
  return index === 0 || index === nodes.length - 1;
}

function constrainDelta(delta) {
  return Math.abs(delta.x) >= Math.abs(delta.y)
    ? { x: delta.x, y: 0 }
    : { x: 0, y: delta.y };
}

function scalePoint(point, amount) {
  return { x: finite(point.x) * amount, y: finite(point.y) * amount };
}

function rotatePoint(point, pivot, radians) {
  const vector = pointSubtract(point, pivot);
  const rotated = rotateVector(vector, radians);
  return pointAdd(pivot, rotated);
}

function rotateVector(vector, radians) {
  const cos = Math.cos(radians);
  const sin = Math.sin(radians);
  return {
    x: finite(vector.x) * cos - finite(vector.y) * sin,
    y: finite(vector.x) * sin + finite(vector.y) * cos,
  };
}

function finite(value) {
  return Number.isFinite(value) ? value : 0;
}

function formatPoint(point) {
  return `${round(point.x)} ${round(point.y)}`;
}

function round(value) {
  return Number.parseFloat(Number(value).toFixed(3));
}

function title(value) {
  return value.replace(/[-_]+/g, " ").replace(/\b\w/g, (c) => c.toUpperCase());
}
