export async function loadCurvePathFromFile(file) {
  if (!file) {
    throw new Error("No curve file selected.");
  }

  const source = await file.text();
  const name = file.name.toLowerCase();

  if (name.endsWith(".bezvg") || name.endsWith(".bezziator") || looksLikeJson(source)) {
    return bezziatorDocumentToPathData(source);
  }

  return svgSourceToPathData(source);
}

export function svgSourceToPathData(source) {
  const trimmed = source.trim();
  if (!trimmed) {
    throw new Error("Curve source is empty.");
  }

  const pathMatch = trimmed.match(/<path\b[^>]*\sd=(["'])(.*?)\1[^>]*>/i);
  if (pathMatch?.[2]) {
    return stripCloseCommand(pathMatch[2]);
  }

  return stripCloseCommand(trimmed);
}

export function bezziatorDocumentToPathData(source) {
  let parsed;
  try {
    parsed = JSON.parse(source);
  } catch {
    throw new Error("Bezziator curve file is not valid JSON.");
  }

  const document = parsed?.format === "bezziator-working-document"
    ? parsed.document
    : parsed.document ?? parsed;
  const paths = Array.isArray(document?.paths) ? document.paths : [];
  const path = paths.find((candidate) => candidate?.closed === false && candidate?.nodes?.length >= 2)
    ?? paths.find((candidate) => candidate?.nodes?.length >= 2);

  if (!path) {
    throw new Error("No usable path found in Bezziator document.");
  }

  return editablePathToSvgPathData(path);
}

function editablePathToSvgPathData(path) {
  const nodes = normalizeConstraintHandles(path);
  if (!Array.isArray(nodes) || nodes.length === 0) {
    throw new Error("Bezziator path has no nodes.");
  }

  const parts = [`M ${formatPoint(nodes[0].position)}`];
  const segmentCount = path.closed ? nodes.length : nodes.length - 1;

  for (let index = 0; index < segmentCount; index += 1) {
    const start = nodes[index];
    const end = nodes[index + 1] ?? nodes[0];
    const c1 = add(start.position, start.handleOut);
    const c2 = add(end.position, end.handleIn);

    if (isLineLike(start, end)) {
      parts.push(`L ${formatPoint(end.position)}`);
    } else {
      parts.push(`C ${formatPoint(c1)} ${formatPoint(c2)} ${formatPoint(end.position)}`);
    }
  }

  return parts.join(" ");
}

function normalizeConstraintHandles(path) {
  const nodes = path.nodes;
  if (!Array.isArray(nodes)) {
    return [];
  }

  return nodes.map((node, index) => {
    if (node?.pairConstraint !== "smooth" && node?.pairConstraint !== "symmetrical") {
      return node;
    }

    const handleIn = vectorOrZero(node.handleIn);
    const handleOut = vectorOrZero(node.handleOut);
    const inLength = vectorLength(handleIn);
    const outLength = vectorLength(handleOut);
    const direction = outLength > 0
      ? normalize(handleOut)
      : inLength > 0
        ? scale(normalize(handleIn), -1)
        : fallbackTangent(path, index);

    if (node.pairConstraint === "symmetrical") {
      const sharedLength = inLength > 0 && outLength > 0
        ? (inLength + outLength) / 2
        : Math.max(inLength, outLength);

      return {
        ...node,
        handleIn: scale(direction, -sharedLength),
        handleOut: scale(direction, sharedLength),
      };
    }

    return {
      ...node,
      handleIn: scale(direction, -inLength),
      handleOut: scale(direction, outLength),
    };
  });
}

function stripCloseCommand(pathData) {
  return pathData.trim().replace(/\s*[zZ]\s*$/u, "");
}

function looksLikeJson(source) {
  return source.trim().startsWith("{");
}

function add(a, b) {
  return {
    x: numberOrZero(a?.x) + numberOrZero(b?.x),
    y: numberOrZero(a?.y) + numberOrZero(b?.y),
  };
}

function subtract(a, b) {
  return {
    x: numberOrZero(a?.x) - numberOrZero(b?.x),
    y: numberOrZero(a?.y) - numberOrZero(b?.y),
  };
}

function scale(vector, amount) {
  return {
    x: numberOrZero(vector?.x) * amount,
    y: numberOrZero(vector?.y) * amount,
  };
}

function normalize(vector) {
  const length = vectorLength(vector);
  return length > 0
    ? { x: vector.x / length, y: vector.y / length }
    : { x: 1, y: 0 };
}

function vectorLength(vector) {
  return Math.hypot(numberOrZero(vector?.x), numberOrZero(vector?.y));
}

function vectorOrZero(vector) {
  return {
    x: numberOrZero(vector?.x),
    y: numberOrZero(vector?.y),
  };
}

function fallbackTangent(path, index) {
  const current = path.nodes[index];
  const previous = previousNode(path, index);
  const next = nextNode(path, index);

  if (previous && next) {
    return normalize(subtract(next.position, previous.position));
  }

  if (next) {
    return normalize(subtract(next.position, current.position));
  }

  if (previous) {
    return normalize(subtract(current.position, previous.position));
  }

  return { x: 1, y: 0 };
}

function previousNode(path, index) {
  if (index > 0) {
    return path.nodes[index - 1];
  }

  return path.closed ? path.nodes.at(-1) : null;
}

function nextNode(path, index) {
  if (index < path.nodes.length - 1) {
    return path.nodes[index + 1];
  }

  return path.closed ? path.nodes[0] : null;
}

function isLineLike(start, end) {
  return start?.handleOutBehavior === "vector" && end?.handleInBehavior === "vector";
}

function formatPoint(point) {
  return `${trimNumber(numberOrZero(point?.x))} ${trimNumber(numberOrZero(point?.y))}`;
}

function numberOrZero(value) {
  return Number.isFinite(value) ? value : 0;
}

function trimNumber(value) {
  const rounded = Math.round(value * 1000) / 1000;
  return Object.is(rounded, -0) ? "0" : String(rounded);
}
