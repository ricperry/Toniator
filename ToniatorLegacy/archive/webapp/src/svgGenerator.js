import { mapPixelToChannels } from "./color.js";
import { pathDataWithEndpointSettings } from "./curvePath.js";
import { getPresetPath } from "./presets.js";
import { calculateGrid, sampleImage } from "./sampling.js";

const CHANNEL_ORDER = ["c", "m", "y", "k"];
const CHANNEL_IDS = {
  c: "toniator-cyan",
  m: "toniator-magenta",
  y: "toniator-yellow",
  k: "toniator-black",
};
const CHANNEL_LABELS = {
  c: "Cyan",
  m: "Magenta",
  y: "Yellow",
  k: "Black",
};
const CHANNEL_SLUGS = {
  c: "cyan",
  m: "magenta",
  y: "yellow",
  k: "black",
};

export function generateHalftoneSvg({
  source,
  grid,
  settings,
  channels,
  includePreviewBackground = false,
  includeBackground = true,
  renderChannelKeys = null,
  sampleProvider = defaultSampleProvider,
}) {
  const { outputWidth, outputHeight } = settings;
  const enabledChannelKeys = CHANNEL_ORDER.filter((key) => channels[key]?.enabled);
  const channelKeysToRender = Array.isArray(renderChannelKeys)
    ? renderChannelKeys
    : enabledChannelKeys;
  const renderSettings = { ...settings, enabledChannelKeys };
  const parts = [
    `<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd" width="${round(outputWidth)}" height="${round(outputHeight)}" viewBox="0 0 ${round(outputWidth)} ${round(outputHeight)}" role="img" aria-label="Vector halftone output">`,
    `<defs><clipPath id="toniator-artboard-clip"><rect x="0" y="0" width="${round(outputWidth)}" height="${round(outputHeight)}"/></clipPath></defs>`,
  ];

  if (includeBackground) {
    parts.push(`<rect width="100%" height="100%" fill="white"/>`);
  }

  const previewSourceUrl = source?.previewDataUrl || source?.dataUrl;
  if (includePreviewBackground && previewSourceUrl) {
    parts.push(
      `<image href="${escapeAttr(previewSourceUrl)}" x="0" y="0" width="${round(outputWidth)}" height="${round(outputHeight)}" preserveAspectRatio="none" opacity="0.22"/>`,
    );
  }

  for (const key of CHANNEL_ORDER) {
    const channel = channels[key];
    if (!channel.enabled) continue;
    if (!channelKeysToRender.includes(key)) continue;

    const channelGeometry = buildChannelGeometry({
      key,
      source,
      settings: renderSettings,
      baseGrid: grid,
      channels,
      channel,
      sampleProvider,
    });

    if (channelGeometry.chunks.length > 0) {
      parts.push(serializeChannelGeometry(channelGeometry));
    }
  }

  parts.push("</svg>");
  return parts.join("\n");
}

function crosshatchLuminanceMode(settings) {
  return settings.valueMode === "crosshatch-luminance";
}

function defaultSampleProvider({ source, grid }) {
  return sampleImage(source, grid.cols, grid.rows);
}

function buildChannelGeometry({
  key,
  source,
  settings,
  baseGrid,
  channels,
  channel,
  sampleProvider,
}) {
  const grid = getChannelGrid({ source, baseGrid, settings, channel });
  const samples = sampleProvider({
    source,
    grid,
    channelKey: key,
    settings,
    channel,
  });
  const d = resolveChannelPath(key, settings, channels);
  const chunks =
    settings.markMode === "curve"
      ? buildCurveChunks({ key, d, samples, grid, settings, channel })
      : buildShapeChunks({ key, d, samples, grid, settings, channel });

  return {
    key,
    id: CHANNEL_IDS[key],
    legacyId: `halftone-${CHANNEL_SLUGS[key]}`,
    label: CHANNEL_LABELS[key],
    color: crosshatchLuminanceMode(settings) ? "#111111" : channel.color,
    opacity: channel.opacity,
    clipToArtboard: settings.markMode === "curve",
    chunks,
  };
}

function serializeChannelGeometry(geometry) {
  const clipAttr = geometry.clipToArtboard ? ` clip-path="url(#toniator-artboard-clip)"` : "";
  const attrs = [
    `id="${escapeAttr(geometry.id)}"`,
    `data-legacy-id="${escapeAttr(geometry.legacyId)}"`,
    `inkscape:groupmode="layer"`,
    `inkscape:label="${escapeAttr(geometry.label)}"`,
    clipAttr.trim(),
    `fill="${escapeAttr(geometry.color)}"`,
    `stroke="${escapeAttr(geometry.color)}"`,
    `opacity="${round(geometry.opacity)}"`,
    `style="mix-blend-mode:multiply"`,
  ].filter(Boolean);
  const parts = [`<g ${attrs.join(" ")}>`];

  for (const chunk of geometry.chunks) {
    const outlinePathData = serializeChunkPathData(chunk);
    if (!outlinePathData) continue;
    const nodeTypes = serializeChunkNodeTypes(chunk);
    const nodeTypesAttr = nodeTypes ? ` sodipodi:nodetypes="${escapeAttr(nodeTypes)}"` : "";

    parts.push(
      `<path id="${escapeAttr(chunk.id)}" d="${escapeAttr(outlinePathData)}" fill-rule="${escapeAttr(chunk.fillRule)}" stroke="none"${nodeTypesAttr}/>`,
    );
  }

  parts.push(`</g>`);
  return parts.join("\n");
}

function serializeChunkPathData(chunk) {
  if (typeof chunk.outlinePathData === "string") {
    return chunk.outlinePathData;
  }

  return (chunk.outlineSegments ?? [])
    .map((segment) => variableWidthCurveToPathData(segment.points, { closed: segment.closed }))
    .filter(Boolean)
    .join(" ");
}

function serializeChunkNodeTypes(chunk) {
  if (chunk.kind !== "centerline") {
    return "";
  }

  return (chunk.outlineSegments ?? [])
    .map((segment) => variableWidthCurveNodeTypes(segment.points, { closed: segment.closed }))
    .join("");
}

export function renderMarkPreview({ mode, d, color, rotation }) {
  const strokeAttrs =
    mode === "curve"
      ? `fill="none" stroke="${escapeAttr(color)}" stroke-width="0.12" stroke-linecap="round" stroke-linejoin="round"`
      : `fill="${escapeAttr(color)}" fill-rule="evenodd" stroke="none"`;

  return `<svg viewBox="-0.7 -0.7 1.4 1.4" aria-hidden="true">
    <path d="${escapeAttr(d)}" transform="rotate(${round(rotation)})" ${strokeAttrs}/>
  </svg>`;
}

function getChannelGrid({ source, baseGrid, settings, channel }) {
  const multiplier = Number.isFinite(channel.resolutionScale)
    ? channel.resolutionScale
    : 1;
  const channelLongEdgeCells = Math.max(
    2,
    Math.round(settings.longEdgeCells * Math.max(0.05, multiplier)),
  );

  if (source) {
    return calculateGrid(
      source,
      settings.outputWidth,
      settings.outputHeight,
      channelLongEdgeCells,
    );
  }

  const cols = Math.max(1, Math.round(baseGrid.cols * multiplier));
  const rows = Math.max(1, Math.round(baseGrid.rows * multiplier));

  return {
    ...baseGrid,
    cols,
    rows,
    cellWidth: settings.outputWidth / cols,
    cellHeight: settings.outputHeight / rows,
  };
}

function buildShapeChunks({ key, d, samples, grid, settings, channel }) {
  const pathFragments = [];
  const ranges = getShapeGridRanges({ grid, settings, channel });

  for (let row = ranges.minRow; row <= ranges.maxRow; row += 1) {
    for (let col = ranges.minCol; col <= ranges.maxCol; col += 1) {
      const placement = getShapePlacement({ col, row, grid, settings, channel });
      if (!placement.visible) continue;
      const value = sampleChannelValue({
        key,
        sample: samples[placement.sampleRow * grid.cols + placement.sampleCol],
        settings,
        channel,
      });

      if (value <= 0) continue;

      pathFragments.push(
        shapeMarkToPathData({
          d,
          value,
          col,
          row,
          grid,
          settings,
          channel,
          placement,
        }),
      );
    }
  }

  if (pathFragments.length === 0) {
    return [];
  }

  return [
    createGeometryChunk({
      key,
      kind: "marks",
      index: 0,
      outlinePathData: pathFragments.join(" "),
      fillRule: "nonzero",
    }),
  ];
}

function buildCurveChunks({ key, d, samples, grid, settings, channel }) {
  const layout = normalizeCurveLayout(settings.curveSpan);

  if (usesCurvePatternLayout(layout, channel)) {
    return buildPatternCurveChunks({ key, d, samples, grid, settings, channel });
  }

  return buildDocumentCurveChunks({ key, d, samples, grid, settings, channel, layout });
}

function buildDocumentCurveChunks({ key, d, samples, grid, settings, channel, layout }) {
  const nodeCount = getDocumentCurveNodeCount(layout, grid, settings, channel);
  const localPoints = samplePathPoints(d, nodeCount);
  const baselineAngle = getCurveBaselineAngle(layout);
  const coverageAngle = baselineAngle + normalizeFinite(channel.gridRotation, 0);
  const channelGridTransform = createDocumentCurveGridTransform(settings, channel, baselineAngle);
  const points = buildFullCurveBaseline(
    localPoints,
    settings,
    channel,
    layout,
    baselineAngle,
    coverageAngle,
  );
  const repeatedPointSets = repeatDocumentCurvePoints(
    points,
    grid,
    settings,
    baselineAngle,
    channelGridTransform,
  ).map((pointSet) => pointSet.map(channelGridTransform));
  const chunks = [];

  repeatedPointSets.forEach((pointSet) => {
    const nodes = pointSet.map((point) => ({
      ...point,
      width: documentCurveWidthAtPoint({
        key,
        point,
        samples,
        grid,
        settings,
        channel,
      }),
    }));

    const connected = curveEndpointsConnected(settings, channel);
    if (connected) {
      if (nodes.some((node) => node.width > 0)) {
        chunks.push(
          createCenterlineChunk({
            key,
            index: chunks.length,
            centerlinePoints: nodes,
            outlineSegments: [
              {
                points: simplifyVariableWidthSegment(nodes, grid, channel),
                closed: true,
              },
            ],
          }),
        );
      }
      return;
    }

    const outlineSegments = activeCurvePathSegments(nodes, { settings, grid, channel })
      .map((segment) => ({ points: segment, closed: false }));

    if (outlineSegments.length > 0) {
      chunks.push(
        createCenterlineChunk({
          key,
          index: chunks.length,
          centerlinePoints: nodes,
          outlineSegments,
        }),
      );
    }
  });

  return chunks;
}

function buildPatternCurveChunks({ key, d, samples, grid, settings, channel }) {
  const localPoints = normalizeMotifPoints(
    samplePathPoints(d, getPatternCurveNodeCount(channel)),
    normalizePositive(channel.curveScale, 32),
  );
  const stacks = buildCurvePatternStacks(localPoints, grid, settings, channel);
  const pointSets = stacks.map((stack) =>
    resampleMotifRowPoints(chainTilePointSets(stack), grid, channel),
  );
  const chunks = [];

  pointSets.forEach((pointSet) => {
    const margin = normalizePositive(channel.curveScale, 32) * 2 + maxCurveWidth(settings, grid, channel);
    if (!pointSetIntersectsArtboard(pointSet, settings, margin)) return;

    const nodes = pointSet.map((point) => ({
      ...point,
      width: documentCurveWidthAtPoint({
        key,
        point,
        samples,
        grid,
        settings,
        channel,
      }),
    }));

    const outlineSegments = activeCurvePathSegments(nodes, { settings, grid, channel })
      .map((segment) => ({ points: segment, closed: false }));

    if (outlineSegments.length > 0) {
      chunks.push(
        createCenterlineChunk({
          key,
          index: chunks.length,
          centerlinePoints: nodes,
          outlineSegments,
        }),
      );
    }
  });

  return chunks;
}

function createCenterlineChunk({ key, index, centerlinePoints, outlineSegments }) {
  return createGeometryChunk({
    key,
    kind: "centerline",
    index,
    centerlinePoints,
    outlineSegments,
    fillRule: "nonzero",
  });
}

function createGeometryChunk({
  key,
  kind,
  index,
  centerlinePoints = [],
  outlinePathData,
  outlineSegments = [],
  fillRule = "nonzero",
}) {
  return {
    id: `${CHANNEL_SLUGS[key]}-${kind}-${String(index + 1).padStart(3, "0")}`,
    channelKey: key,
    kind,
    centerlinePoints,
    outlinePathData,
    outlineSegments,
    fillRule,
    bounds: outlinePathData
      ? getPathDataBounds(outlinePathData)
      : getVariableWidthSegmentsBounds(outlineSegments),
  };
}

function resolveChannelPath(key, settings, channels) {
  const resolve = (path, channel) => normalizePathData(path, settings, channel);

  const channel = channels[key];

  if (settings.markMode === "curve") {
    return resolve(
      settings.syncCurveChannels
        ? settings.sharedPath.trim() || getPresetPath("curve", settings.sharedPreset)
        : channel.customPath?.trim() ||
        settings.sharedPath.trim() ||
        getPresetPath("curve", channel.preset || settings.sharedPreset),
      channel,
    );
  }

  if (settings.useSharedMark) {
    return resolve(
      settings.sharedPath.trim() || getPresetPath(settings.markMode, settings.sharedPreset),
      channel,
    );
  }

  return resolve(
    channel.customPath.trim() || getPresetPath(settings.markMode, channel.preset),
    channel,
  );
}

function normalizePathData(path, settings, channel) {
  if (settings.markMode !== "curve") {
    return path;
  }

  const match = path.match(/<path\b[^>]*\sd=(["'])(.*?)\1[^>]*>/i);
  const pathData = (match?.[2] ?? path).trim().replace(/\s*[zZ]\s*$/u, "");
  if (usesCurvePatternLayout(normalizeCurveLayout(settings.curveSpan), channel)) {
    return pathData;
  }

  return pathDataWithEndpointSettings(pathData, {
    connectEndpoints: curveEndpointsConnected(settings, channel),
    smoothSeamTangents: curveSeamSmooth(settings, channel),
  });
}

function sampleChannelValue({ key, sample, settings, channel }) {
  const values = mapPixelToChannels(
    sample,
    settings.valueMode,
    settings.singleChannel,
    settings.enabledChannelKeys,
  );
  const rawValue = values[key] ?? 0;
  return mapThreshold(rawValue, channel.threshold);
}

function getMarkMetrics({
  value,
  col,
  row,
  grid,
  settings,
  channel,
  centerOverride = null,
}) {
  const cellSize = Math.min(grid.cellWidth, grid.cellHeight);
  const centerX = centerOverride?.x ?? (col + 0.5) * grid.cellWidth + channel.offsetX;
  const centerY = centerOverride?.y ?? (row + 0.5) * grid.cellHeight + channel.offsetY;
  const gridScale = settings.gridScale / 100;
  const min = Math.max(0, settings.minMark / 100);
  const max = Math.max(min, settings.maxMark / 100) * (channel.maxSize / 100);
  const size = cellSize * (min + (max - min) * value) * channel.scale;

  return {
    cellSize,
    centerX,
    centerY,
    gridScale,
    size,
    rotation: channel.rotation,
  };
}

function shapeMarkToPathData({ d, value, col, row, grid, settings, channel, placement }) {
  const metrics = getMarkMetrics({
    value,
    col,
    row,
    grid,
    settings,
    channel,
    centerOverride: placement,
  });
  const shapeScale = metrics.size * metrics.gridScale;

  return transformPathData(d, {
    translateX: metrics.centerX,
    translateY: metrics.centerY,
    rotation: metrics.rotation,
    scale: shapeScale,
  });
}

function transformPathData(d, transform) {
  const tokens = tokenizePathData(d);
  const commands = [];
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
    return relative ? addPoints(current, point) : point;
  };
  const moveTo = (point) => {
    commands.push(`M ${formatPoint(transformPathPoint(point, transform))}`);
    current = point;
    subpathStart = point;
  };
  const lineTo = (point) => {
    commands.push(`L ${formatPoint(transformPathPoint(point, transform))}`);
    current = point;
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
      moveTo(readPoint(relative));
      clearControls();
      while (hasNumber()) {
        lineTo(readPoint(relative));
      }
      continue;
    }

    if (lower === "l") {
      while (hasNumber()) lineTo(readPoint(relative));
      clearControls();
      continue;
    }

    if (lower === "h") {
      while (hasNumber()) {
        const x = readNumber();
        lineTo({ x: relative ? current.x + x : x, y: current.y });
      }
      clearControls();
      continue;
    }

    if (lower === "v") {
      while (hasNumber()) {
        const y = readNumber();
        lineTo({ x: current.x, y: relative ? current.y + y : y });
      }
      clearControls();
      continue;
    }

    if (lower === "c") {
      while (hasNumber()) {
        const c1 = readPoint(relative);
        const c2 = readPoint(relative);
        const end = readPoint(relative);
        commands.push(`C ${formatPoint(transformPathPoint(c1, transform))} ${formatPoint(transformPathPoint(c2, transform))} ${formatPoint(transformPathPoint(end, transform))}`);
        current = end;
        lastCubicControl = c2;
        lastQuadraticControl = null;
      }
      continue;
    }

    if (lower === "s") {
      while (hasNumber()) {
        const c1 = lastCubicControl ? reflectPoint(lastCubicControl, current) : current;
        const c2 = readPoint(relative);
        const end = readPoint(relative);
        commands.push(`C ${formatPoint(transformPathPoint(c1, transform))} ${formatPoint(transformPathPoint(c2, transform))} ${formatPoint(transformPathPoint(end, transform))}`);
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
        commands.push(`Q ${formatPoint(transformPathPoint(control, transform))} ${formatPoint(transformPathPoint(end, transform))}`);
        current = end;
        lastQuadraticControl = control;
        lastCubicControl = null;
      }
      continue;
    }

    if (lower === "t") {
      while (hasNumber()) {
        const control = lastQuadraticControl ? reflectPoint(lastQuadraticControl, current) : current;
        const end = readPoint(relative);
        commands.push(`Q ${formatPoint(transformPathPoint(control, transform))} ${formatPoint(transformPathPoint(end, transform))}`);
        current = end;
        lastQuadraticControl = control;
        lastCubicControl = null;
      }
      continue;
    }

    if (lower === "a") {
      while (hasNumber()) {
        // Arc geometry is uncommon for Toniator presets. Preserve continuity by
        // converting unsupported transformed arcs to a line endpoint.
        readNumber();
        readNumber();
        readNumber();
        readNumber();
        readNumber();
        lineTo(readPoint(relative));
      }
      clearControls();
      continue;
    }

    if (lower === "z") {
      commands.push("Z");
      if (subpathStart) current = subpathStart;
      clearControls();
      continue;
    }

    break;
  }

  return commands.join(" ");
}

function transformPathPoint(point, transform) {
  const scaled = {
    x: point.x * transform.scale,
    y: point.y * transform.scale,
  };
  const radians = (transform.rotation * Math.PI) / 180;
  const cos = Math.cos(radians);
  const sin = Math.sin(radians);
  return {
    x: transform.translateX + scaled.x * cos - scaled.y * sin,
    y: transform.translateY + scaled.x * sin + scaled.y * cos,
  };
}

function formatPoint(point) {
  return `${round(point.x)} ${round(point.y)}`;
}

function getShapePlacement({ col, row, grid, settings, channel }) {
  const artboardWidth = grid.cols * grid.cellWidth;
  const artboardHeight = grid.rows * grid.cellHeight;
  const phaseX = wrapSignedGridOffset(channel.offsetX, grid.cellWidth);
  const phaseY = wrapSignedGridOffset(channel.offsetY, grid.cellHeight);
  const gridRotation = normalizeFinite(channel.gridRotation, 0);
  const logicalX = gridRotation === 0
    ? positiveModulo((col + 0.5) * grid.cellWidth + phaseX, artboardWidth)
    : (col + 0.5) * grid.cellWidth + phaseX;
  const logicalY = gridRotation === 0
    ? positiveModulo((row + 0.5) * grid.cellHeight + phaseY, artboardHeight)
    : (row + 0.5) * grid.cellHeight + phaseY;
  const point = applyChannelGridTransform({ x: logicalX, y: logicalY }, settings, channel);
  const margin = maxShapeExtent(settings, grid, channel);

  return {
    x: point.x,
    y: point.y,
    visible:
      point.x >= -margin &&
      point.x <= settings.outputWidth + margin &&
      point.y >= -margin &&
      point.y <= settings.outputHeight + margin,
    sampleCol: clampIndex(Math.floor(point.x / grid.cellWidth), grid.cols),
    sampleRow: clampIndex(Math.floor(point.y / grid.cellHeight), grid.rows),
  };
}

function getShapeGridRanges({ grid, settings, channel }) {
  const gridRotation = normalizeFinite(channel.gridRotation, 0);
  if (Math.abs(gridRotation) <= 0.0001) {
    return {
      minCol: 0,
      maxCol: grid.cols - 1,
      minRow: 0,
      maxRow: grid.rows - 1,
    };
  }

  const margin = maxShapeExtent(settings, grid, channel);
  const pivot = channelGridPivot(settings, channel);
  const corners = [
    { x: -margin, y: -margin },
    { x: settings.outputWidth + margin, y: -margin },
    { x: settings.outputWidth + margin, y: settings.outputHeight + margin },
    { x: -margin, y: settings.outputHeight + margin },
  ].map((point) => rotatePointAround(point, pivot, -gridRotation));
  const bounds = getPointBounds(corners);
  const phaseX = wrapSignedGridOffset(channel.offsetX, grid.cellWidth);
  const phaseY = wrapSignedGridOffset(channel.offsetY, grid.cellHeight);

  return {
    minCol: Math.floor((bounds.minX - phaseX) / grid.cellWidth - 0.5),
    maxCol: Math.ceil((bounds.maxX - phaseX) / grid.cellWidth - 0.5),
    minRow: Math.floor((bounds.minY - phaseY) / grid.cellHeight - 0.5),
    maxRow: Math.ceil((bounds.maxY - phaseY) / grid.cellHeight - 0.5),
  };
}

function maxShapeExtent(settings, grid, channel) {
  return (maxCurveWidth(settings, grid, channel) * (settings.gridScale / 100)) / 2;
}

function wrapSignedGridOffset(offset, spacing) {
  if (!Number.isFinite(offset) || !Number.isFinite(spacing) || spacing <= 0) {
    return 0;
  }

  return positiveModulo(offset + spacing / 2, spacing) - spacing / 2;
}

function clampIndex(index, length) {
  return Math.min(length - 1, Math.max(0, index));
}

function repeatDocumentCurvePoints(points, grid, settings, baselineAngle, visibilityTransform = (point) => point) {
  const spacing = Math.max(1, Math.min(grid.cellWidth, grid.cellHeight));
  const repeatRadius = Math.ceil(
    Math.hypot(settings.outputWidth, settings.outputHeight) / spacing,
  ) + 2;
  const normal = screenNormalFromRotation(baselineAngle);
  const repeated = [];

  for (let index = -repeatRadius; index <= repeatRadius; index += 1) {
    const offset = index * spacing;
    const copy = points.map((point) => ({
      x: point.x + normal.x * offset,
      y: point.y + normal.y * offset,
    }));

    const visibleCopy = copy.map(visibilityTransform);
    if (pointSetIntersectsArtboard(visibleCopy, settings, spacing * 2)) {
      repeated.push(copy);
    }
  }

  return repeated;
}

function createDocumentCurveGridTransform(settings, channel, baselineAngle) {
  const gridRotation = normalizeFinite(channel.gridRotation, 0);
  if (Math.abs(gridRotation) <= 0.0001) {
    return (point) => point;
  }

  const pivot = channelGridPivot(settings, channel);
  const pageCenter = {
    x: settings.outputWidth / 2,
    y: settings.outputHeight / 2,
  };
  const finalTangent = unitVector(baselineAngle + gridRotation);
  const rotatedCenter = rotatePointAround(pageCenter, pivot, gridRotation);
  const centerShift = {
    x: rotatedCenter.x - pageCenter.x,
    y: rotatedCenter.y - pageCenter.y,
  };
  const tangentShift =
    centerShift.x * finalTangent.x + centerShift.y * finalTangent.y;

  return (point) => {
    const rotated = rotatePointAround(point, pivot, gridRotation);
    return {
      x: rotated.x - finalTangent.x * tangentShift,
      y: rotated.y - finalTangent.y * tangentShift,
    };
  };
}

function screenNormalFromRotation(rotation) {
  const radians = (rotation * Math.PI) / 180;
  return {
    x: -Math.sin(radians),
    y: Math.cos(radians),
  };
}

function pointSetIntersectsArtboard(points, settings, margin) {
  const bounds = getPointBounds(points);
  return (
    bounds.maxX >= -margin &&
    bounds.minX <= settings.outputWidth + margin &&
    bounds.maxY >= -margin &&
    bounds.minY <= settings.outputHeight + margin
  );
}

function documentCurveWidthAtPoint({ key, point, samples, grid, settings, channel }) {
  const value = sampleInterpolatedChannelValue({
    key,
    point,
    samples,
    grid,
    settings,
    channel,
  });

  if (value <= 0) {
    return 0;
  }

  return curveWidthFromValue(value, grid, settings, channel);
}

function sampleInterpolatedChannelValue({ key, point, samples, grid, settings, channel }) {
  const x = point.x / grid.cellWidth - 0.5;
  const y = point.y / grid.cellHeight - 0.5;
  const x0 = Math.floor(x);
  const y0 = Math.floor(y);
  const tx = clamp01(x - x0);
  const ty = clamp01(y - y0);
  const rows = [];

  for (let rowOffset = -1; rowOffset <= 2; rowOffset += 1) {
    rows.push(
      cubicInterpolate(
        sampleRawChannelValueAtCell({ key, col: x0 - 1, row: y0 + rowOffset, samples, grid, settings }),
        sampleRawChannelValueAtCell({ key, col: x0, row: y0 + rowOffset, samples, grid, settings }),
        sampleRawChannelValueAtCell({ key, col: x0 + 1, row: y0 + rowOffset, samples, grid, settings }),
        sampleRawChannelValueAtCell({ key, col: x0 + 2, row: y0 + rowOffset, samples, grid, settings }),
        tx,
      ),
    );
  }

  return mapThreshold(
    clamp01(cubicInterpolate(rows[0], rows[1], rows[2], rows[3], ty)),
    channel.threshold,
  );
}

function sampleRawChannelValueAtCell({ key, col, row, samples, grid, settings }) {
  const clampedCol = clampIndex(col, grid.cols);
  const clampedRow = clampIndex(row, grid.rows);
  const values = mapPixelToChannels(
    samples[clampedRow * grid.cols + clampedCol],
    settings.valueMode,
    settings.singleChannel,
    settings.enabledChannelKeys,
  );

  return values[key] ?? 0;
}

function curveWidthFromValue(value, grid, settings, channel) {
  const cellSize = Math.min(grid.cellWidth, grid.cellHeight);
  const min = Math.max(0, settings.minMark / 100);
  const max = Math.max(min, settings.maxMark / 100) * (channel.maxSize / 100);
  return cellSize * (min + (max - min) * value) * channel.scale;
}

function normalizeCurveLayout(layout) {
  if (layout === "document-width") return "full-width";
  if (layout === "document-height") return "full-height";
  if (layout === "document-fit") return "full-width";
  if (layout === "cell-chain" || layout === "tiled-width" || layout === "tiled-height") {
    return "motif-pattern";
  }
  return layout || "full-width";
}

function usesCurvePatternLayout(layout) {
  return layout === "motif-pattern";
}

function getDocumentCurveNodeCount(layout, grid, settings, channel) {
  const cellSize = Math.max(1, Math.min(grid.cellWidth, grid.cellHeight));
  const quality = normalizeOutputQuality(channel);

  const dimension = layout.endsWith("height")
    ? settings.outputHeight
    : settings.outputWidth;
  return Math.max(2, Math.ceil((dimension / cellSize) * quality));
}

function getCurveBaselineAngle(layout) {
  const dimensionAngle = layout.endsWith("height") ? 90 : 0;
  return dimensionAngle;
}

function buildFullCurveBaseline(localPoints, settings, channel, layout, baselineAngle, coverageAngle) {
  const targetLength = artboardProjectionSpan(settings, coverageAngle);
  const scaled = scaleCurveToLength(localPoints, targetLength);
  const bounds = getPointBounds(scaled);
  const centered = scaled.map((point) => ({
    x: point.x - bounds.minX + (settings.outputWidth - bounds.width) / 2,
    y: point.y - bounds.minY + (settings.outputHeight - bounds.height) / 2,
  }));

  return centered.map((point) => {
    const rotated = rotatePoint(
      point,
      settings.outputWidth / 2,
      settings.outputHeight / 2,
      baselineAngle,
    );

    return {
      x: rotated.x + channel.offsetX,
      y: rotated.y + channel.offsetY,
    };
  });
}

function getPatternCurveNodeCount(channel) {
  return Math.max(4, Math.ceil(24 * normalizeOutputQuality(channel)));
}

function normalizeMotifPoints(points, curveScale) {
  if (points.length < 2) {
    return [
      { x: -curveScale / 2, y: 0 },
      { x: curveScale / 2, y: 0 },
    ];
  }

  const bounds = getPointBounds(points);
  const sourceSize = Math.max(bounds.width, bounds.height, 0.0001);
  const center = {
    x: bounds.minX + bounds.width / 2,
    y: bounds.minY + bounds.height / 2,
  };
  const scale = curveScale / sourceSize;

  return points.map((point) => ({
    x: (point.x - center.x) * scale,
    y: (point.y - center.y) * scale,
  }));
}

function buildCurvePatternStacks(localPoints, grid, settings, channel) {
  const markRotation = 0;
  const forwardTile = transformMotifTile(localPoints, { rotation: markRotation });
  const rowAdvance = motifRowAdvance(forwardTile, channel);
  const tileAdvanceLength = Math.hypot(rowAdvance.x, rowAdvance.y);
  const autoCoverage = motifCoverageMode(channel) === "auto";
  const autoCounts = autoCoverage
    ? computeMotifAutoCounts({
        settings,
        channel,
        grid,
        motifRadius: motifPointRadius(localPoints),
        tileAdvanceLength,
      })
    : null;
  const tileCount = autoCounts?.tileCount ?? normalizeInteger(channel.tileCount, 1, 10000, 1);
  const stackCount = autoCounts?.stackCount ?? normalizeInteger(channel.stackCount, 1, 10000, 1);
  const stackSpacing = normalizeFinite(channel.stackSpacing, 36);
  const stackDirection = unitVector(motifStackAngle(channel));
  const stackVector = scaleVector(stackDirection, stackSpacing);
  const tileOffset = normalizeFinite(channel.tileOffset, 0);
  const stackOffset = normalizeFinite(channel.stackOffset, 0);
  const alternateStackOffset = normalizeFinite(channel.alternateStackOffset, 0);
  const tileDirection = normalizeVector(rowAdvance);
  const tileOrigin = (tileCount - 1) / 2;
  const stackOrigin = (stackCount - 1) / 2;
  const center = {
    x: settings.outputWidth / 2 + normalizeFinite(channel.offsetX, 0),
    y: settings.outputHeight / 2 + normalizeFinite(channel.offsetY, 0),
  };
  const stacks = [];

  for (let stackIndex = 0; stackIndex < stackCount; stackIndex += 1) {
    const stack = [];
    let rowAnchor = null;
    for (let tileIndex = 0; tileIndex < tileCount; tileIndex += 1) {
      const alternate = (tileIndex + stackIndex) % 2 === 1;
      const tile = orientMotifTileForRow(
        transformMotifTile(localPoints, {
          rotation: markRotation,
          flip: alternate && channel.alternateTileTransform === "flip",
          rotate180: alternate && channel.alternateTileTransform === "rotate-180",
        }),
        rowAdvance,
      );
      const instanceCenter = {
        x:
          center.x +
          (stackIndex - stackOrigin) * stackVector.x +
          tileDirection.x * tileOffset +
          stackDirection.x * stackOffset +
          (stackIndex % 2 === 1 ? tileDirection.x * alternateStackOffset : 0),
        y:
          center.y +
          (stackIndex - stackOrigin) * stackVector.y +
          tileDirection.y * tileOffset +
          stackDirection.y * stackOffset +
          (stackIndex % 2 === 1 ? tileDirection.y * alternateStackOffset : 0),
      };
      if (!rowAnchor) {
        rowAnchor = {
          x: instanceCenter.x - tileOrigin * rowAdvance.x,
          y: instanceCenter.y - tileOrigin * rowAdvance.y,
        };
      }
      const tileStart = tile[0] ?? { x: 0, y: 0 };
      const tileOriginPoint = {
        x: rowAnchor.x - tileStart.x,
        y: rowAnchor.y - tileStart.y,
      };
      const placedTile = tile.map((point) =>
        applyChannelGridTransform(
          {
            x: tileOriginPoint.x + point.x,
            y: tileOriginPoint.y + point.y,
          },
          settings,
          channel,
        ),
      );

      stack.push(placedTile);
      rowAnchor = {
        x: tileOriginPoint.x + (tile.at(-1)?.x ?? tileStart.x),
        y: tileOriginPoint.y + (tile.at(-1)?.y ?? tileStart.y),
      };
    }
    stacks.push(stack);
  }

  return stacks;
}

function transformMotifTile(points, options) {
  return points.map((point) =>
    transformMotifPoint(point, options),
  );
}

function tileEndpointAdvance(tile) {
  const start = tile[0] ?? { x: 0, y: 0 };
  const end = tile.at(-1) ?? start;
  return {
    x: end.x - start.x,
    y: end.y - start.y,
  };
}

function motifTileDirection(tile, fallbackAngle) {
  const advance = tileEndpointAdvance(tile);
  const length = Math.hypot(advance.x, advance.y);
  return length > 0.0001
    ? { x: advance.x / length, y: advance.y / length }
    : unitVector(fallbackAngle);
}

function motifRowAdvance(tile, channel) {
  const advance = tileEndpointAdvance(tile);
  const length = Math.hypot(advance.x, advance.y);

  if (length > 0.0001) {
    return advance;
  }

  return scaleVector(
    unitVector(normalizeFinite(channel.tileAngle, 0)),
    normalizePositive(channel.curveScale, 32),
  );
}

function orientMotifTileForRow(tile, rowAdvance) {
  const advance = tileEndpointAdvance(tile);
  const dot = advance.x * rowAdvance.x + advance.y * rowAdvance.y;

  return dot < 0 ? [...tile].reverse() : tile;
}

export function computeMotifAutoCounts({
  settings,
  channel,
  grid,
  motifRadius = null,
  tileAdvanceLength = null,
}) {
  const cellSize = Math.max(1, Math.min(grid?.cellWidth || 1, grid?.cellHeight || 1));
  const bleedCells = normalizeFinite(channel.motifBleed, 2);
  const bleed = Math.max(0, bleedCells) * cellSize;
  const radius =
    Number.isFinite(motifRadius) && motifRadius > 0
      ? motifRadius
      : Math.max(1, normalizePositive(channel.curveScale, 32) / 2);
  const gridRotation = normalizeFinite(channel.gridRotation, 0);
  const nominalTile = transformMotifTile(
    [
      { x: -normalizePositive(channel.curveScale, 32) / 2, y: 0 },
      { x: normalizePositive(channel.curveScale, 32) / 2, y: 0 },
    ],
    { rotation: 0 },
  );
  const tileDirection = rotateVector(
    motifTileDirection(nominalTile, normalizeFinite(channel.tileAngle, 0)),
    gridRotation,
  );
  const stackDirection = unitVector(motifStackAngle(channel) + gridRotation);
  const tileSpacing = Math.max(
    1,
    Number.isFinite(tileAdvanceLength) && tileAdvanceLength > 0
      ? tileAdvanceLength
      : normalizePositive(channel.curveScale, 32),
  );
  const stackSpacing = Math.max(1, Math.abs(normalizeFinite(channel.stackSpacing, 36)));
  const margin = bleed + radius * 2 + Math.hypot(settings.outputWidth, settings.outputHeight) * 0.08;
  const coveragePad = Math.max(4, Math.ceil(Math.max(0, bleedCells)));

  return {
    tileCount: clampInteger(
      Math.ceil((artboardProjectionForVector(settings, tileDirection) + margin * 2) / tileSpacing) +
        coveragePad,
      1,
      10000,
    ),
    stackCount: clampInteger(
      Math.ceil((artboardProjectionForVector(settings, stackDirection) + margin * 2) / stackSpacing) +
        coveragePad,
      1,
      10000,
    ),
  };
}

function motifCoverageMode(channel) {
  return channel.motifCoverageMode || channel.coverageMode || "manual";
}

function motifStackAngle(channel) {
  return normalizeFinite(channel.tileAngle, 0) + 90 + normalizeFinite(channel.stackAngle, 0);
}

function artboardProjectionForVector(settings, vector) {
  return Math.abs(settings.outputWidth * vector.x) + Math.abs(settings.outputHeight * vector.y);
}

function motifPointRadius(points) {
  return Math.max(
    1,
    ...points.map((point) => Math.hypot(point.x, point.y)),
  );
}

function chainTilePointSets(tilePointSets) {
  const chained = [];

  for (const tile of tilePointSets) {
    for (let index = 0; index < tile.length; index += 1) {
      const point = tile[index];
      const previous = chained.at(-1);
      if (previous && index === 0 && squaredDistance(previous, point) <= 0.000001) {
        continue;
      }
      chained.push(point);
    }
  }

  return chained;
}

function resampleMotifRowPoints(points, grid, channel) {
  if (points.length < 2) {
    return points;
  }

  const quality = normalizeOutputQuality(channel);
  const cellSize = Math.max(1, Math.min(grid.cellWidth, grid.cellHeight));
  const targetSpacing = Math.max(0.5, cellSize / Math.max(1, quality));
  const length = polylineLength(points);
  const count = clampInteger(
    Math.ceil(length / targetSpacing) + 1,
    points.length,
    20000,
  );

  return samplePolylinePoints(points, count);
}

function polylineLength(points) {
  let length = 0;

  for (let index = 1; index < points.length; index += 1) {
    length += Math.sqrt(squaredDistance(points[index - 1], points[index]));
  }

  return length;
}

function transformMotifPoint(point, options = {}) {
  const source = {
    x: options.flip ? -point.x : point.x,
    y: point.y,
  };
  const rotatedAlternate = options.rotate180
    ? { x: -source.x, y: -source.y }
    : source;
  const rotated = rotateVector(rotatedAlternate, options.rotation);

  return rotated;
}

function maxCurveWidth(settings, grid, channel) {
  const cellSize = Math.min(grid.cellWidth, grid.cellHeight);
  const min = Math.max(0, settings.minMark / 100);
  const max = Math.max(min, settings.maxMark / 100) * (channel.maxSize / 100);
  return cellSize * (min + (max - min)) * channel.scale;
}

function normalizeOutputQuality(channel) {
  return normalizePositive(channel.outputQuality, 1);
}

function normalizePositive(value, fallback) {
  const number = Number(value);
  return Number.isFinite(number) && number > 0 ? number : fallback;
}

function normalizeFinite(value, fallback) {
  const number = Number(value);
  return Number.isFinite(number) ? number : fallback;
}

function normalizeInteger(value, min, max, fallback) {
  const number = Math.round(Number(value));
  if (!Number.isFinite(number)) return fallback;
  return Math.min(max, Math.max(min, number));
}

function clampInteger(value, min, max) {
  return Math.min(max, Math.max(min, Math.round(value)));
}

function clamp01(value) {
  return Math.min(1, Math.max(0, value));
}

function cubicInterpolate(p0, p1, p2, p3, amount) {
  const t2 = amount * amount;
  const t3 = t2 * amount;

  return 0.5 * (
    2 * p1 +
    (-p0 + p2) * amount +
    (2 * p0 - 5 * p1 + 4 * p2 - p3) * t2 +
    (-p0 + 3 * p1 - 3 * p2 + p3) * t3
  );
}

function unitVector(angle) {
  const radians = (angle * Math.PI) / 180;
  return {
    x: Math.cos(radians),
    y: Math.sin(radians),
  };
}

function scaleVector(vector, amount) {
  return {
    x: vector.x * amount,
    y: vector.y * amount,
  };
}

function rotateVector(point, rotation) {
  const radians = (rotation * Math.PI) / 180;
  const cos = Math.cos(radians);
  const sin = Math.sin(radians);

  return {
    x: point.x * cos - point.y * sin,
    y: point.x * sin + point.y * cos,
  };
}

function applyChannelGridTransform(point, settings, channel) {
  const gridRotation = normalizeFinite(channel.gridRotation, 0);
  if (Math.abs(gridRotation) <= 0.0001) {
    return point;
  }

  return rotatePointAround(point, channelGridPivot(settings, channel), gridRotation);
}

function channelGridPivot(settings, channel) {
  return {
    x: settings.outputWidth / 2 + normalizeFinite(channel.gridPivotX, 0),
    y: settings.outputHeight / 2 + normalizeFinite(channel.gridPivotY, 0),
  };
}

function rotatePointAround(point, pivot, rotation) {
  const rotated = rotateVector(
    {
      x: point.x - pivot.x,
      y: point.y - pivot.y,
    },
    rotation,
  );

  return {
    x: pivot.x + rotated.x,
    y: pivot.y + rotated.y,
  };
}

function scaleCurveToLength(points, targetLength) {
  const endpointLength = endpointDistance(points);
  if (endpointLength > 0.0001) {
    return scaleCurveForEndpointTiling(points, targetLength);
  }

  const box = getPointBounds(points);
  const sourceLength = box.width || endpointDistance(points) || 1;
  const scale = targetLength / sourceLength;

  return points.map((point) => ({
    x: (point.x - box.minX) * scale,
    y: (point.y - (box.minY + box.height / 2)) * scale,
  }));
}

function artboardProjectionSpan(settings, angle) {
  const radians = (angle * Math.PI) / 180;
  return (
    Math.abs(settings.outputWidth * Math.cos(radians)) +
    Math.abs(settings.outputHeight * Math.sin(radians))
  );
}

function scaleCurveForEndpointTiling(points, targetLength) {
  if (points.length < 2) {
    return [{ x: 0, y: 0 }, { x: targetLength, y: 0 }];
  }

  const start = points[0];
  const end = points.at(-1);
  const sourceLength = Math.hypot(end.x - start.x, end.y - start.y);

  if (sourceLength <= 0.0001) {
    const fallback = scaleCurveToLengthByBounds(points, targetLength);
    const fallbackStart = fallback[0];
    return fallback.map((point) => ({
      x: point.x - fallbackStart.x,
      y: point.y - fallbackStart.y,
    }));
  }

  const scale = targetLength / sourceLength;
  return points.map((point) => ({
    x: (point.x - start.x) * scale,
    y: (point.y - start.y) * scale,
  }));
}

function scaleCurveToLengthByBounds(points, targetLength) {
  const box = getPointBounds(points);
  const sourceLength = box.width || box.height || 1;
  const scale = targetLength / sourceLength;

  return points.map((point) => ({
    x: (point.x - box.minX) * scale,
    y: (point.y - (box.minY + box.height / 2)) * scale,
  }));
}

function endpointDistance(points) {
  if (points.length < 2) return 0;
  return Math.hypot(
    points.at(-1).x - points[0].x,
    points.at(-1).y - points[0].y,
  );
}

function variableWidthCurveToSvg(points, { closed = false } = {}) {
  const pathData = variableWidthCurveToPathData(points, { closed });
  return pathData ? `<path d="${pathData}" stroke="none" fill-rule="${closed ? "evenodd" : "nonzero"}"/>` : "";
}

function variableWidthCurveToPathData(points, { closed = false } = {}) {
  const sourcePoints = closed ? removeDuplicateClosingPoint(points) : points;
  if (sourcePoints.length < 2) {
    return "";
  }

  const { left, right } = variableWidthOutlinePoints(sourcePoints, { closed });

  if (closed) {
    return [
      smoothBoundaryPath(left, { closed: true }),
      smoothBoundaryPath([...right].reverse(), { closed: true }),
    ].join(" ");
  }

  const startCollapsed = samePoint(left[0], right[0]);
  const endCollapsed = samePoint(left.at(-1), right.at(-1));
  const rightBoundary = [...right].reverse();

  const commands = [
    smoothBoundaryPath(left),
    endCollapsed ? "" : capCurvePath(left.at(-1), right.at(-1), sourcePoints.at(-1)),
    smoothBoundaryPath(rightBoundary, { skipMove: true }),
    startCollapsed ? "" : capCurvePath(right[0], left[0], sourcePoints[0]),
    "Z",
  ];

  return commands.filter(Boolean).join(" ");
}

function variableWidthOutlinePoints(sourcePoints, { closed = false } = {}) {
  const left = [];
  const right = [];

  for (let index = 0; index < sourcePoints.length; index += 1) {
    const point = sourcePoints[index];
    const prev = closed
      ? sourcePoints[(index - 1 + sourcePoints.length) % sourcePoints.length]
      : sourcePoints[Math.max(0, index - 1)];
    const next = closed
      ? sourcePoints[(index + 1) % sourcePoints.length]
      : sourcePoints[Math.min(sourcePoints.length - 1, index + 1)];
    const tangent = normalizeVector({
      x: next.x - prev.x,
      y: next.y - prev.y,
    });
    const normal = { x: -tangent.y, y: tangent.x };
    const halfWidth = point.width / 2;

    left.push({
      x: point.x + normal.x * halfWidth,
      y: point.y + normal.y * halfWidth,
    });
    right.push({
      x: point.x - normal.x * halfWidth,
      y: point.y - normal.y * halfWidth,
    });
  }

  return { left, right };
}

function variableWidthCurveNodeTypes(points, { closed = false } = {}) {
  const sourcePoints = closed ? removeDuplicateClosingPoint(points) : points;
  if (sourcePoints.length < 2) {
    return "";
  }

  if (closed) {
    return closedRailNodeTypes(sourcePoints.length) + closedRailNodeTypes(sourcePoints.length);
  }

  const outline = variableWidthOutlinePoints(sourcePoints);
  return openOutlineNodeTypes(sourcePoints.length, {
    startCollapsed: samePoint(outline.left[0], outline.right[0]),
    endCollapsed: samePoint(outline.left.at(-1), outline.right.at(-1)),
  });
}

function closedRailNodeTypes(pointCount) {
  if (pointCount <= 0) return "";
  return "s".repeat(pointCount + 1);
}

function openOutlineNodeTypes(pointCount, { startCollapsed = false, endCollapsed = false } = {}) {
  if (pointCount <= 1) return "";
  const leftTypes = ["c"];
  for (let index = 1; index < pointCount - 1; index += 1) {
    leftTypes.push("s");
  }
  leftTypes.push("c");

  const rightTypes = [];
  const firstRightIndex = endCollapsed ? pointCount - 2 : pointCount - 1;
  const lastRightIndex = 0;
  for (let index = firstRightIndex; index >= lastRightIndex; index -= 1) {
    if (!endCollapsed && index === pointCount - 1) {
      rightTypes.push("c");
    } else if (!startCollapsed && index === 0) {
      rightTypes.push("c");
    } else {
      rightTypes.push("s");
    }
  }

  return [
    leftTypes.join(""),
    rightTypes.join(""),
  ].join("");
}

function activeCurvePathSegments(nodes, { settings, grid, channel }) {
  const margin = maxCurveWidth(settings, grid, channel) * 1.5 + 2;
  return splitActiveCurveSegments(nodes)
    .flatMap((segment) => clipVariableWidthSegmentToArtboard(segment, settings, margin))
    .map((segment) => simplifyVariableWidthSegment(segment, grid, channel))
    .filter((segment) => segment.length >= 2);
}

function clipVariableWidthSegmentToArtboard(points, settings, margin) {
  if (points.length < 2) return [];

  const bounds = {
    minX: -margin,
    minY: -margin,
    maxX: settings.outputWidth + margin,
    maxY: settings.outputHeight + margin,
  };
  const clippedSegments = [];
  let current = [];

  for (let index = 1; index < points.length; index += 1) {
    const clipped = clipLineSegmentToBounds(points[index - 1], points[index], bounds);
    if (!clipped) {
      if (current.length >= 2) clippedSegments.push(current);
      current = [];
      continue;
    }

    if (current.length === 0 || !sameVariableWidthPoint(current.at(-1), clipped.start)) {
      if (current.length >= 2) clippedSegments.push(current);
      current = [clipped.start];
    }

    current.push(clipped.end);
  }

  if (current.length >= 2) clippedSegments.push(current);
  return clippedSegments;
}

function clipLineSegmentToBounds(start, end, bounds) {
  const dx = end.x - start.x;
  const dy = end.y - start.y;
  let t0 = 0;
  let t1 = 1;

  const tests = [
    [-dx, start.x - bounds.minX],
    [dx, bounds.maxX - start.x],
    [-dy, start.y - bounds.minY],
    [dy, bounds.maxY - start.y],
  ];

  for (const [p, q] of tests) {
    if (Math.abs(p) <= 0.000001) {
      if (q < 0) return null;
      continue;
    }

    const r = q / p;
    if (p < 0) {
      if (r > t1) return null;
      t0 = Math.max(t0, r);
    } else {
      if (r < t0) return null;
      t1 = Math.min(t1, r);
    }
  }

  if (t0 > t1) return null;
  return {
    start: interpolateVariableWidthPoint(start, end, t0),
    end: interpolateVariableWidthPoint(start, end, t1),
  };
}

function interpolateVariableWidthPoint(start, end, amount) {
  return {
    x: start.x + (end.x - start.x) * amount,
    y: start.y + (end.y - start.y) * amount,
    width: start.width + ((end.width ?? 0) - (start.width ?? 0)) * amount,
  };
}

function sameVariableWidthPoint(a, b) {
  return (
    a &&
    b &&
    Math.abs(a.x - b.x) <= 0.001 &&
    Math.abs(a.y - b.y) <= 0.001 &&
    Math.abs((a.width ?? 0) - (b.width ?? 0)) <= 0.001
  );
}

function simplifyVariableWidthSegment(points, grid, channel) {
  if (points.length <= 3) return points;

  const tolerance = variableWidthSimplificationTolerance(grid, channel);
  const keep = new Set([0, points.length - 1]);
  simplifyVariableWidthRange(points, 0, points.length - 1, tolerance, keep);

  return [...keep]
    .sort((a, b) => a - b)
    .map((index) => points[index]);
}

function simplifyVariableWidthRange(points, startIndex, endIndex, tolerance, keep) {
  if (endIndex <= startIndex + 1) return;

  let farthestIndex = -1;
  let farthestDistance = 0;

  for (let index = startIndex + 1; index < endIndex; index += 1) {
    const distance = variableWidthPointDistanceToSegment(
      points[index],
      points[startIndex],
      points[endIndex],
    );
    if (distance > farthestDistance) {
      farthestDistance = distance;
      farthestIndex = index;
    }
  }

  if (farthestDistance <= tolerance || farthestIndex < 0) return;

  keep.add(farthestIndex);
  simplifyVariableWidthRange(points, startIndex, farthestIndex, tolerance, keep);
  simplifyVariableWidthRange(points, farthestIndex, endIndex, tolerance, keep);
}

function variableWidthPointDistanceToSegment(point, start, end) {
  const dw = (end.width ?? 0) - (start.width ?? 0);
  const dx = end.x - start.x;
  const dy = end.y - start.y;
  const lengthSquared = dx * dx + dy * dy + dw * dw;

  if (lengthSquared <= 0.000001) {
    return Math.hypot(point.x - start.x, point.y - start.y, (point.width ?? 0) - (start.width ?? 0));
  }

  const t = clamp01(
    ((point.x - start.x) * dx +
      (point.y - start.y) * dy +
      ((point.width ?? 0) - (start.width ?? 0)) * dw) /
      lengthSquared,
  );
  const projected = interpolateVariableWidthPoint(start, end, t);

  return Math.hypot(
    point.x - projected.x,
    point.y - projected.y,
    (point.width ?? 0) - (projected.width ?? 0),
  );
}

function variableWidthSimplificationTolerance(grid, channel) {
  const cellSize = Math.max(1, Math.min(grid.cellWidth, grid.cellHeight));
  const quality = Math.sqrt(normalizeOutputQuality(channel));
  return Math.min(0.75, Math.max(0.15, (cellSize * 0.04) / quality));
}

function removeDuplicateClosingPoint(points) {
  if (points.length < 3) return points;
  const first = points[0];
  const last = points.at(-1);
  if (Math.hypot(first.x - last.x, first.y - last.y) <= 0.001) {
    return points.slice(0, -1);
  }
  return points;
}

function smoothBoundaryPath(points, { closed = false, skipMove = false } = {}) {
  if (points.length === 0) return "";
  if (points.length === 1) return skipMove ? "" : `M ${round(points[0].x)} ${round(points[0].y)}`;

  const commands = skipMove ? [] : [`M ${round(points[0].x)} ${round(points[0].y)}`];
  const segmentCount = closed ? points.length : points.length - 1;

  for (let index = 0; index < segmentCount; index += 1) {
    const { c1, c2, end } = smoothBoundarySegmentControls(points, index, { closed });
    commands.push(`C ${round(c1.x)} ${round(c1.y)} ${round(c2.x)} ${round(c2.y)} ${round(end.x)} ${round(end.y)}`);
  }

  if (closed) {
    commands.push("Z");
  }

  return commands.join(" ");
}

function smoothBoundarySegmentControls(points, index, { closed = false } = {}) {
  const start = points[index];
  const end = points[(index + 1) % points.length];
  const startTangent = smoothBoundaryTangent(points, index, { closed });
  const endTangent = smoothBoundaryTangent(points, (index + 1) % points.length, { closed });
  const segmentLength = Math.hypot(end.x - start.x, end.y - start.y);
  const startHandleLength = Math.min(
    segmentLength / 3,
    adjacentBoundaryDistance(points, index, 1, { closed }) / 3,
  );
  const endHandleLength = Math.min(
    segmentLength / 3,
    adjacentBoundaryDistance(points, (index + 1) % points.length, -1, { closed }) / 3,
  );

  return {
    c1: {
      x: start.x + startTangent.x * startHandleLength,
      y: start.y + startTangent.y * startHandleLength,
    },
    c2: {
      x: end.x - endTangent.x * endHandleLength,
      y: end.y - endTangent.y * endHandleLength,
    },
    end,
  };
}

function smoothBoundaryTangent(points, index, { closed = false } = {}) {
  const previousIndex = closed
    ? (index - 1 + points.length) % points.length
    : Math.max(0, index - 1);
  const nextIndex = closed
    ? (index + 1) % points.length
    : Math.min(points.length - 1, index + 1);
  const previous = points[previousIndex];
  const next = points[nextIndex];

  return normalizeVector({
    x: next.x - previous.x,
    y: next.y - previous.y,
  });
}

function adjacentBoundaryDistance(points, index, direction, { closed = false } = {}) {
  const adjacentIndex = closed
    ? positiveModulo(index + direction, points.length)
    : Math.min(points.length - 1, Math.max(0, index + direction));
  const adjacent = points[adjacentIndex];
  const point = points[index];
  return Math.hypot(adjacent.x - point.x, adjacent.y - point.y);
}

function capCurvePath(from, to, center) {
  if (!from || !to || !center) return "";
  const c1 = {
    x: from.x + ((center.x - from.x) * 2) / 3,
    y: from.y + ((center.y - from.y) * 2) / 3,
  };
  const c2 = {
    x: to.x + ((center.x - to.x) * 2) / 3,
    y: to.y + ((center.y - to.y) * 2) / 3,
  };
  return `C ${round(c1.x)} ${round(c1.y)} ${round(c2.x)} ${round(c2.y)} ${round(to.x)} ${round(to.y)}`;
}

function splitActiveCurveSegments(nodes) {
  const segments = [];
  let current = [];
  let previousZero = null;

  for (const node of nodes) {
    if (node.width > 0) {
      if (current.length === 0 && previousZero) {
        current.push({ ...previousZero, width: 0 });
      }
      current.push(node);
      continue;
    }

    const zeroNode = { ...node, width: 0 };
    if (current.length > 0) {
      current.push(zeroNode);
      segments.push(current);
      current = [];
    }
    previousZero = zeroNode;
  }

  if (current.length > 0) {
    segments.push(current);
  }

  return segments;
}

function samplePathPoints(d, count) {
  const browserPoints = sampleBrowserPathPoints(d, count);
  if (browserPoints.length >= 2) {
    return browserPoints;
  }

  const parsedPoints = sampleParsedPathPoints(d, count);
  if (parsedPoints.length >= 2) {
    return parsedPoints;
  }

  return samplePolylinePoints(extractPathCoordinatePairs(d), count);
}

function sampleBrowserPathPoints(d, count) {
  if (typeof document === "undefined") {
    return [];
  }

  try {
    const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
    path.setAttribute("d", d);
    svg.append(path);
    document.body.append(svg);
    const totalLength = path.getTotalLength();
    const points = [];

    if (totalLength > 0) {
      for (let index = 0; index < count; index += 1) {
        const distance = (totalLength * index) / Math.max(1, count - 1);
        const point = path.getPointAtLength(distance);
        points.push({ x: point.x, y: point.y });
      }
    }

    svg.remove();
    return points;
  } catch {
    return [];
  }
}

function extractPathCoordinatePairs(d) {
  const numbers = d.match(/[-+]?(?:\d*\.)?\d+(?:e[-+]?\d+)?/gi)?.map(Number) ?? [];
  const points = [];

  for (let index = 0; index + 1 < numbers.length; index += 2) {
    points.push({ x: numbers[index], y: numbers[index + 1] });
  }

  return points.length >= 2 ? points : [{ x: -0.5, y: 0 }, { x: 0.5, y: 0 }];
}

function sampleParsedPathPoints(d, count) {
  const polyline = flattenPathData(d);
  return polyline.length >= 2 ? samplePolylinePoints(polyline, count) : [];
}

function flattenPathData(d) {
  const tokens = tokenizePathData(d);
  const points = [];
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
    return relative ? addPoints(current, point) : point;
  };
  const appendPoint = (point) => {
    current = point;
    points.push(point);
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
      const point = readPoint(relative);
      appendPoint(point);
      subpathStart = point;
      clearControls();

      while (hasNumber()) {
        appendPoint(readPoint(relative));
      }
      continue;
    }

    if (lower === "l") {
      while (hasNumber()) {
        appendPoint(readPoint(relative));
      }
      clearControls();
      continue;
    }

    if (lower === "h") {
      while (hasNumber()) {
        const x = readNumber();
        appendPoint({ x: relative ? current.x + x : x, y: current.y });
      }
      clearControls();
      continue;
    }

    if (lower === "v") {
      while (hasNumber()) {
        const y = readNumber();
        appendPoint({ x: current.x, y: relative ? current.y + y : y });
      }
      clearControls();
      continue;
    }

    if (lower === "c") {
      while (hasNumber()) {
        const c1 = readPoint(relative);
        const c2 = readPoint(relative);
        const end = readPoint(relative);
        appendCubicSamples(points, current, c1, c2, end);
        current = end;
        lastCubicControl = c2;
        lastQuadraticControl = null;
      }
      continue;
    }

    if (lower === "s") {
      while (hasNumber()) {
        const c1 = lastCubicControl ? reflectPoint(lastCubicControl, current) : current;
        const c2 = readPoint(relative);
        const end = readPoint(relative);
        appendCubicSamples(points, current, c1, c2, end);
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
        appendQuadraticSamples(points, current, control, end);
        current = end;
        lastQuadraticControl = control;
        lastCubicControl = null;
      }
      continue;
    }

    if (lower === "t") {
      while (hasNumber()) {
        const control = lastQuadraticControl ? reflectPoint(lastQuadraticControl, current) : current;
        const end = readPoint(relative);
        appendQuadraticSamples(points, current, control, end);
        current = end;
        lastQuadraticControl = control;
        lastCubicControl = null;
      }
      continue;
    }

    if (lower === "z") {
      if (subpathStart) {
        appendPoint(subpathStart);
      }
      clearControls();
      continue;
    }

    break;
  }

  return points;
}

function tokenizePathData(d) {
  return d.match(/[a-zA-Z]|[-+]?(?:\d*\.)?\d+(?:e[-+]?\d+)?/gi) ?? [];
}

function isPathCommand(token) {
  return /^[a-zA-Z]$/u.test(token);
}

function appendCubicSamples(points, p0, p1, p2, p3) {
  const steps = 24;

  for (let step = 1; step <= steps; step += 1) {
    const t = step / steps;
    points.push(cubicPoint(p0, p1, p2, p3, t));
  }
}

function appendQuadraticSamples(points, p0, p1, p2) {
  const c1 = {
    x: p0.x + (p1.x - p0.x) * (2 / 3),
    y: p0.y + (p1.y - p0.y) * (2 / 3),
  };
  const c2 = {
    x: p2.x + (p1.x - p2.x) * (2 / 3),
    y: p2.y + (p1.y - p2.y) * (2 / 3),
  };
  appendCubicSamples(points, p0, c1, c2, p2);
}

function cubicPoint(p0, p1, p2, p3, t) {
  const mt = 1 - t;
  return {
    x: mt ** 3 * p0.x + 3 * mt ** 2 * t * p1.x + 3 * mt * t ** 2 * p2.x + t ** 3 * p3.x,
    y: mt ** 3 * p0.y + 3 * mt ** 2 * t * p1.y + 3 * mt * t ** 2 * p2.y + t ** 3 * p3.y,
  };
}

function reflectPoint(point, center) {
  return {
    x: center.x * 2 - point.x,
    y: center.y * 2 - point.y,
  };
}

function addPoints(a, b) {
  return {
    x: a.x + b.x,
    y: a.y + b.y,
  };
}

function samplePolylinePoints(points, count) {
  if (points.length < 2) {
    return points;
  }

  const lengths = [0];
  let totalLength = 0;

  for (let index = 1; index < points.length; index += 1) {
    totalLength += Math.sqrt(squaredDistance(points[index - 1], points[index]));
    lengths.push(totalLength);
  }

  if (totalLength === 0) {
    return Array.from({ length: count }, () => ({ ...points[0] }));
  }

  const sampled = [];

  for (let sampleIndex = 0; sampleIndex < count; sampleIndex += 1) {
    const target = (totalLength * sampleIndex) / Math.max(1, count - 1);
    let segmentIndex = 1;

    while (segmentIndex < lengths.length - 1 && lengths[segmentIndex] < target) {
      segmentIndex += 1;
    }

    const start = points[segmentIndex - 1];
    const end = points[segmentIndex];
    const segmentLength = lengths[segmentIndex] - lengths[segmentIndex - 1] || 1;
    const t = (target - lengths[segmentIndex - 1]) / segmentLength;

    sampled.push({
      x: start.x + (end.x - start.x) * t,
      y: start.y + (end.y - start.y) * t,
    });
  }

  return sampled;
}

function getPointBounds(points) {
  return points.reduce(
    (bounds, point) => ({
      minX: Math.min(bounds.minX, point.x),
      minY: Math.min(bounds.minY, point.y),
      maxX: Math.max(bounds.maxX, point.x),
      maxY: Math.max(bounds.maxY, point.y),
      width: Math.max(bounds.maxX, point.x) - Math.min(bounds.minX, point.x),
      height: Math.max(bounds.maxY, point.y) - Math.min(bounds.minY, point.y),
    }),
    {
      minX: Infinity,
      minY: Infinity,
      maxX: -Infinity,
      maxY: -Infinity,
      width: 0,
      height: 0,
    },
  );
}

function getPathDataBounds(d) {
  const numbers = d
    .match(/[-+]?(?:\d*\.)?\d+(?:e[-+]?\d+)?/gi)
    ?.map(Number)
    .filter(Number.isFinite) ?? [];
  const points = [];

  for (let index = 0; index + 1 < numbers.length; index += 2) {
    points.push({ x: numbers[index], y: numbers[index + 1] });
  }

  return points.length > 0 ? getPointBounds(points) : null;
}

function getVariableWidthSegmentsBounds(segments) {
  const points = segments.flatMap((segment) => segment.points ?? []);
  return points.length > 0 ? getPointBounds(points) : null;
}

function rotatePoint(point, centerX, centerY, rotation) {
  const radians = (rotation * Math.PI) / 180;
  const cos = Math.cos(radians);
  const sin = Math.sin(radians);
  const x = point.x - centerX;
  const y = point.y - centerY;

  return {
    x: centerX + x * cos - y * sin,
    y: centerY + x * sin + y * cos,
  };
}

function normalizeVector(vector) {
  const length = Math.hypot(vector.x, vector.y) || 1;
  return {
    x: vector.x / length,
    y: vector.y / length,
  };
}

function inGrid(col, row, grid) {
  return col >= 0 && row >= 0 && col < grid.cols && row < grid.rows;
}

function squaredDistance(a, b) {
  return (a.x - b.x) ** 2 + (a.y - b.y) ** 2;
}

function samePoint(a, b, tolerance = 0.001) {
  return Boolean(a && b && Math.hypot(a.x - b.x, a.y - b.y) <= tolerance);
}

function positiveModulo(value, divisor) {
  return ((value % divisor) + divisor) % divisor;
}

function curveEndpointsConnected(settings, channel) {
  return settings.syncCurveChannels
    ? Boolean(settings.sharedConnectEndpoints)
    : Boolean(channel.connectEndpoints);
}

function curveSeamSmooth(settings, channel) {
  return settings.syncCurveChannels
    ? Boolean(settings.sharedSmoothSeamTangents)
    : Boolean(channel.smoothSeamTangents);
}

function mapThreshold(value, threshold) {
  if (value < threshold) return 0;
  if (threshold >= 0.999) return value >= threshold ? 1 : 0;
  return (value - threshold) / (1 - threshold);
}

function round(value) {
  return Number.parseFloat(Number(value).toFixed(3));
}

function escapeAttr(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll('"', "&quot;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}
