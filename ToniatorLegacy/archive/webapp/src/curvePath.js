const EPSILON = 1e-6;

export function pathDataToEditableCurve(pathData) {
  const tokens = tokenizePathData(pathData);
  const nodes = [];
  let index = 0;
  let command = "";
  let current = { x: 0, y: 0 };
  let subpathStart = null;
  let lastCubicControl = null;
  let lastQuadraticControl = null;

  const hasNumber = () => index < tokens.length && !isPathCommand(tokens[index]);
  const readNumber = () => Number(tokens[index++]);
  const readPoint = (relative) => {
    const point = { x: readNumber(), y: readNumber() };
    return relative ? add(current, point) : point;
  };
  const ensureCurrentNode = () => {
    if (nodes.length === 0) {
      nodes.push(createNode(current));
    }
    return nodes.at(-1);
  };
  const clearControls = () => {
    lastCubicControl = null;
    lastQuadraticControl = null;
  };

  while (index < tokens.length) {
    if (isPathCommand(tokens[index])) {
      command = tokens[index++];
    }

    const lower = command.toLowerCase();
    const relative = command === lower;

    if (lower === "m") {
      if (!hasNumber()) continue;
      current = readPoint(relative);
      nodes.push(createNode(current));
      subpathStart = current;
      clearControls();

      while (hasNumber()) {
        const end = readPoint(relative);
        appendLine(nodes, current, end);
        current = end;
      }
      continue;
    }

    if (lower === "l") {
      while (hasNumber()) {
        const end = readPoint(relative);
        appendLine(nodes, current, end);
        current = end;
      }
      clearControls();
      continue;
    }

    if (lower === "h") {
      while (hasNumber()) {
        const x = readNumber();
        const end = { x: relative ? current.x + x : x, y: current.y };
        appendLine(nodes, current, end);
        current = end;
      }
      clearControls();
      continue;
    }

    if (lower === "v") {
      while (hasNumber()) {
        const y = readNumber();
        const end = { x: current.x, y: relative ? current.y + y : y };
        appendLine(nodes, current, end);
        current = end;
      }
      clearControls();
      continue;
    }

    if (lower === "c") {
      while (hasNumber()) {
        const c1 = readPoint(relative);
        const c2 = readPoint(relative);
        const end = readPoint(relative);
        appendCubic(nodes, current, c1, c2, end);
        current = end;
        lastCubicControl = c2;
        lastQuadraticControl = null;
      }
      continue;
    }

    if (lower === "s") {
      while (hasNumber()) {
        const c1 = lastCubicControl ? reflect(lastCubicControl, current) : current;
        const c2 = readPoint(relative);
        const end = readPoint(relative);
        appendCubic(nodes, current, c1, c2, end);
        current = end;
        lastCubicControl = c2;
        lastQuadraticControl = null;
      }
      continue;
    }

    if (lower === "q") {
      while (hasNumber()) {
        const control = readPoint(relative);
        const end = readPoint(relative);
        appendQuadratic(nodes, current, control, end);
        current = end;
        lastQuadraticControl = control;
        lastCubicControl = null;
      }
      continue;
    }

    if (lower === "t") {
      while (hasNumber()) {
        const control = lastQuadraticControl ? reflect(lastQuadraticControl, current) : current;
        const end = readPoint(relative);
        appendQuadratic(nodes, current, control, end);
        current = end;
        lastQuadraticControl = control;
        lastCubicControl = null;
      }
      continue;
    }

    if (lower === "z") {
      if (subpathStart && nodes.length > 1 && distance(current, subpathStart) > EPSILON) {
        appendLine(nodes, current, subpathStart);
      }
      clearControls();
      continue;
    }

    ensureCurrentNode();
    break;
  }

  return {
    nodes: inferNodeTypes(nodes),
  };
}

export function editableCurveToPathData(curve, options = {}) {
  const nodes = normalizedCurveNodes(curve, options);
  if (nodes.length === 0) return "";

  const parts = [`M ${formatPoint(nodes[0].position)}`];
  const segmentCount = options.connectEndpoints ? nodes.length : nodes.length - 1;

  for (let index = 0; index < segmentCount; index += 1) {
    const start = nodes[index];
    const end = nodes[index + 1] ?? nodes[0];
    if (!end) break;

    if (isLineLike(start, end)) {
      parts.push(`L ${formatPoint(end.position)}`);
      continue;
    }

    parts.push(
      `C ${formatPoint(add(start.position, start.handleOut))} ${formatPoint(add(end.position, end.handleIn))} ${formatPoint(end.position)}`,
    );
  }

  return parts.join(" ");
}

export function pathDataWithEndpointSettings(pathData, options = {}) {
  if (!options.connectEndpoints && !options.smoothSeamTangents) {
    return stripCloseCommand(pathData);
  }

  return editableCurveToPathData(pathDataToEditableCurve(pathData), options);
}

export function normalizedCurveNodes(curve, options = {}) {
  const nodes = (curve.nodes ?? []).map(cloneNode);
  applyNodeConstraints(nodes);

  if (options.connectEndpoints && options.smoothSeamTangents) {
    applySeamTangentContinuity(nodes, "start-out");
  }

  return nodes;
}

export function applySeamTangentContinuity(nodes, leadingSide = "start-out") {
  if (nodes.length < 2) return;

  const start = nodes[0];
  const end = nodes.at(-1);
  const startLength = vectorLength(start.handleOut);
  const endLength = vectorLength(end.handleIn);

  if (leadingSide === "end-in" && endLength > EPSILON) {
    const direction = normalize(scale(end.handleIn, -1));
    start.handleOut = scale(direction, startLength || endLength);
    return;
  }

  if (startLength > EPSILON) {
    const direction = normalize(start.handleOut);
    end.handleIn = scale(direction, -(endLength || startLength));
    return;
  }

  if (endLength > EPSILON) {
    const direction = normalize(scale(end.handleIn, -1));
    start.handleOut = scale(direction, startLength || endLength);
  }
}

export function setNodeType(curve, nodeIndex, type) {
  const nodes = (curve.nodes ?? []).map(cloneNode);
  const node = nodes[nodeIndex];
  if (!node) return { nodes };

  if (type === "vector") {
    const previous = nodes[nodeIndex - 1];
    const next = nodes[nodeIndex + 1];
    node.handleIn = previous ? scale(subtract(previous.position, node.position), 1 / 3) : zero();
    node.handleOut = next ? scale(subtract(next.position, node.position), 1 / 3) : zero();
    node.handleInBehavior = previous ? "vector" : "free";
    node.handleOutBehavior = next ? "vector" : "free";
    node.pairConstraint = "independent";
    return { nodes };
  }

  node.handleInBehavior = "free";
  node.handleOutBehavior = "free";
  node.pairConstraint = type === "smooth" || type === "symmetrical" ? type : "independent";
  applyNodeConstraint(nodes, nodeIndex);

  return { nodes };
}

export function curveBounds(curve) {
  const nodes = curve.nodes ?? [];
  const points = nodes.flatMap((node) => [
    node.position,
    add(node.position, node.handleIn),
    add(node.position, node.handleOut),
  ]);

  if (points.length === 0) {
    return { minX: -0.5, minY: -0.5, maxX: 0.5, maxY: 0.5, width: 1, height: 1 };
  }

  const bounds = points.reduce(
    (acc, point) => ({
      minX: Math.min(acc.minX, point.x),
      minY: Math.min(acc.minY, point.y),
      maxX: Math.max(acc.maxX, point.x),
      maxY: Math.max(acc.maxY, point.y),
    }),
    { minX: Infinity, minY: Infinity, maxX: -Infinity, maxY: -Infinity },
  );
  bounds.width = Math.max(EPSILON, bounds.maxX - bounds.minX);
  bounds.height = Math.max(EPSILON, bounds.maxY - bounds.minY);
  return bounds;
}

export function pointAdd(a, b) {
  return add(a, b);
}

export function pointSubtract(a, b) {
  return subtract(a, b);
}

export function pointScale(vector, amount) {
  return scale(vector, amount);
}

export function pointLength(vector) {
  return vectorLength(vector);
}

function appendLine(nodes, from, to) {
  const previous = nodes.at(-1) ?? createNode(from);
  if (nodes.length === 0) nodes.push(previous);

  previous.handleOut = scale(subtract(to, from), 1 / 3);
  previous.handleOutBehavior = "vector";

  const node = createNode(to);
  node.handleIn = scale(subtract(from, to), 1 / 3);
  node.handleInBehavior = "vector";
  nodes.push(node);
}

function appendCubic(nodes, from, c1, c2, end) {
  const previous = nodes.at(-1) ?? createNode(from);
  if (nodes.length === 0) nodes.push(previous);

  previous.handleOut = subtract(c1, previous.position);
  previous.handleOutBehavior = "free";

  const node = createNode(end);
  node.handleIn = subtract(c2, end);
  node.handleInBehavior = "free";
  nodes.push(node);
}

function appendQuadratic(nodes, current, control, end) {
  appendCubic(
    nodes,
    current,
    add(current, scale(subtract(control, current), 2 / 3)),
    add(end, scale(subtract(control, end), 2 / 3)),
    end,
  );
}

function inferNodeTypes(nodes) {
  return nodes.map((node) => {
    const bothVector = node.handleInBehavior === "vector" && node.handleOutBehavior === "vector";
    if (bothVector) {
      return { ...node, pairConstraint: "independent" };
    }

    const inLength = vectorLength(node.handleIn);
    const outLength = vectorLength(node.handleOut);
    if (inLength > EPSILON && outLength > EPSILON) {
      const cross = node.handleIn.x * node.handleOut.y - node.handleIn.y * node.handleOut.x;
      const dot = node.handleIn.x * node.handleOut.x + node.handleIn.y * node.handleOut.y;
      if (Math.abs(cross) <= inLength * outLength * 1e-4 && dot < 0) {
        return {
          ...node,
          pairConstraint: Math.abs(inLength - outLength) <= Math.max(inLength, outLength) * 1e-4
            ? "symmetrical"
            : "smooth",
        };
      }
    }

    return node;
  });
}

function applyNodeConstraints(nodes) {
  for (let index = 0; index < nodes.length; index += 1) {
    applyNodeConstraint(nodes, index);
  }
}

function applyNodeConstraint(nodes, index) {
  const node = nodes[index];
  if (!node || (node.pairConstraint !== "smooth" && node.pairConstraint !== "symmetrical")) {
    return;
  }

  const inLength = vectorLength(node.handleIn);
  const outLength = vectorLength(node.handleOut);
  const fallback = fallbackTangent(nodes, index);
  const direction =
    outLength > EPSILON
      ? normalize(node.handleOut)
      : inLength > EPSILON
        ? normalize(scale(node.handleIn, -1))
        : fallback;

  if (node.pairConstraint === "symmetrical") {
    const sharedLength = inLength > EPSILON && outLength > EPSILON
      ? (inLength + outLength) / 2
      : Math.max(inLength, outLength);
    node.handleIn = scale(direction, -sharedLength);
    node.handleOut = scale(direction, sharedLength);
    return;
  }

  node.handleIn = scale(direction, -inLength);
  node.handleOut = scale(direction, outLength);
}

function fallbackTangent(nodes, index) {
  const current = nodes[index];
  const previous = nodes[index - 1];
  const next = nodes[index + 1];

  if (previous && next) return normalize(subtract(next.position, previous.position));
  if (next) return normalize(subtract(next.position, current.position));
  if (previous) return normalize(subtract(current.position, previous.position));
  return { x: 1, y: 0 };
}

function isLineLike(start, end) {
  return start.handleOutBehavior === "vector" && end.handleInBehavior === "vector";
}

function tokenizePathData(pathData) {
  return String(pathData).match(/[a-zA-Z]|[-+]?(?:\d*\.)?\d+(?:e[-+]?\d+)?/gi) ?? [];
}

function isPathCommand(token) {
  return /^[a-zA-Z]$/u.test(token);
}

function stripCloseCommand(pathData) {
  return String(pathData).trim().replace(/\s*[zZ]\s*$/u, "");
}

function createNode(position) {
  return {
    position: { x: finite(position.x), y: finite(position.y) },
    handleIn: zero(),
    handleOut: zero(),
    handleInBehavior: "free",
    handleOutBehavior: "free",
    pairConstraint: "independent",
  };
}

function cloneNode(node) {
  return {
    position: clonePoint(node.position),
    handleIn: clonePoint(node.handleIn),
    handleOut: clonePoint(node.handleOut),
    handleInBehavior: node.handleInBehavior ?? "free",
    handleOutBehavior: node.handleOutBehavior ?? "free",
    pairConstraint: node.pairConstraint ?? "independent",
  };
}

function clonePoint(point) {
  return {
    x: finite(point?.x),
    y: finite(point?.y),
  };
}

function zero() {
  return { x: 0, y: 0 };
}

function add(a, b) {
  return { x: finite(a?.x) + finite(b?.x), y: finite(a?.y) + finite(b?.y) };
}

function subtract(a, b) {
  return { x: finite(a?.x) - finite(b?.x), y: finite(a?.y) - finite(b?.y) };
}

function scale(vector, amount) {
  return { x: finite(vector?.x) * amount, y: finite(vector?.y) * amount };
}

function reflect(point, center) {
  return { x: center.x * 2 - point.x, y: center.y * 2 - point.y };
}

function distance(a, b) {
  return Math.hypot(finite(a?.x) - finite(b?.x), finite(a?.y) - finite(b?.y));
}

function vectorLength(vector) {
  return Math.hypot(finite(vector?.x), finite(vector?.y));
}

function normalize(vector) {
  const length = vectorLength(vector);
  return length > EPSILON ? { x: vector.x / length, y: vector.y / length } : { x: 1, y: 0 };
}

function finite(value) {
  return Number.isFinite(value) ? value : 0;
}

function formatPoint(point) {
  return `${trimNumber(point.x)} ${trimNumber(point.y)}`;
}

function trimNumber(value) {
  const rounded = Math.round(finite(value) * 1000) / 1000;
  return Object.is(rounded, -0) ? "0" : String(rounded);
}
