import { mapPixelToChannels } from "./color.js";
import { pathDataWithEndpointSettings } from "./curvePath.js";
import { getPresetPath } from "./presets.js";
import { calculateGrid, sampleImage } from "./sampling.js";

const CHANNEL_ORDER = ["c", "m", "y", "k"];
const CHANNEL_IDS = {
  c: "halftone-cyan",
  m: "halftone-magenta",
  y: "halftone-yellow",
  k: "halftone-black",
};

export function generateHalftoneSvg({
  source,
  grid,
  settings,
  channels,
  includePreviewBackground = false,
  sampleProvider = defaultSampleProvider,
}) {
  const { outputWidth, outputHeight, markMode } = settings;
  const enabledChannelKeys = CHANNEL_ORDER.filter((key) => channels[key]?.enabled);
  const renderSettings = { ...settings, enabledChannelKeys };
  const parts = [
    `<svg xmlns="http://www.w3.org/2000/svg" width="${round(outputWidth)}" height="${round(outputHeight)}" viewBox="0 0 ${round(outputWidth)} ${round(outputHeight)}" role="img" aria-label="Vector halftone output">`,
    `<defs><clipPath id="toniator-artboard-clip"><rect x="0" y="0" width="${round(outputWidth)}" height="${round(outputHeight)}"/></clipPath></defs>`,
    `<rect width="100%" height="100%" fill="white"/>`,
  ];

  const previewSourceUrl = source?.previewDataUrl || source?.dataUrl;
  if (includePreviewBackground && previewSourceUrl) {
    parts.push(
      `<image href="${escapeAttr(previewSourceUrl)}" x="0" y="0" width="${round(outputWidth)}" height="${round(outputHeight)}" preserveAspectRatio="none" opacity="0.22"/>`,
    );
  }

  for (const key of CHANNEL_ORDER) {
    const channel = channels[key];
    if (!channel.enabled) continue;

    const channelGrid = getChannelGrid({ source, baseGrid: grid, settings: renderSettings, channel });
    const channelSamples = sampleProvider({
      source,
      grid: channelGrid,
      channelKey: key,
      settings: renderSettings,
      channel,
    });
    const markPath = resolveChannelPath(key, renderSettings, channels);
    const pathFragments =
      markMode === "curve"
        ? buildCurveElements({
            key,
            d: markPath,
            samples: channelSamples,
            grid: channelGrid,
            settings: renderSettings,
            channel,
          })
        : buildShapeElements({
            key,
            d: markPath,
            samples: channelSamples,
            grid: channelGrid,
            settings: renderSettings,
            channel,
          });

    if (pathFragments.length > 0) {
      const clipAttr = markMode === "curve" ? ` clip-path="url(#toniator-artboard-clip)"` : "";
      const renderColor = crosshatchLuminanceMode(renderSettings) ? "#111111" : channel.color;
      parts.push(
        `<g id="${CHANNEL_IDS[key]}"${clipAttr} fill="${escapeAttr(renderColor)}" stroke="${escapeAttr(renderColor)}" opacity="${round(channel.opacity)}" style="mix-blend-mode:multiply">`,
        `<path d="${escapeAttr(pathFragments.join(" "))}" fill-rule="nonzero" stroke="none"/>`,
        `</g>`,
      );
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

function buildShapeElements({ key, d, samples, grid, settings, channel }) {
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

  return pathFragments;
}

function buildCurveElements({ key, d, samples, grid, settings, channel }) {
  const layout = normalizeCurveLayout(settings.curveSpan);

  if (usesCurvePatternLayout(layout, channel)) {
    return buildPatternCurveElements({ key, d, samples, grid, settings, channel });
  }

  return buildDocumentCurveElements({ key, d, samples, grid, settings, channel, layout });
}

function buildDocumentCurveElements({ key, d, samples, grid, settings, channel, layout }) {
  const nodeCount = getDocumentCurveNodeCount(layout, grid, settings, channel);
  const localPoints = samplePathPoints(d, nodeCount);
  const baselineAngle = getCurveBaselineAngle(layout, channel.rotation);
  const points = layout.startsWith("tiled")
    ? buildTiledCurveBaseline(localPoints, grid, settings, channel, layout, baselineAngle)
    : buildFullCurveBaseline(localPoints, settings, channel, layout, baselineAngle);
  const repeatedPointSets = repeatDocumentCurvePoints(
    points,
    grid,
    settings,
    baselineAngle,
  ).map((pointSet) =>
    pointSet.map((point) => applyChannelGridTransform(point, settings, channel)),
  );

  return repeatedPointSets.flatMap((pointSet) => {
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
      return nodes.some((node) => node.width > 0)
        ? [variableWidthCurveToPathData(nodes, { closed: true })]
        : [];
    }

    const segments = splitActiveCurveSegments(nodes);

    return segments
      .filter((segment) => segment.length >= 2)
      .map((segment) =>
        variableWidthCurveToPathData(segment, {
          closed:
            connected &&
            segments.length === 1 &&
            segment.length === nodes.length,
        }),
      );
  });
}

function buildPatternCurveElements({ key, d, samples, grid, settings, channel }) {
  const localPoints = normalizeMotifPoints(
    samplePathPoints(d, getPatternCurveNodeCount(channel)),
    normalizePositive(channel.curveScale, 32),
  );
  const stacks = buildCurvePatternStacks(localPoints, settings, channel);
  const connected = curveEndpointsConnected(settings, channel);
  const pointSets = connected
    ? stacks.map((stack) => chainTilePointSets(stack))
    : stacks.flat();

  return pointSets.flatMap((pointSet) => {
    const margin = normalizePositive(channel.curveScale, 32) * 2 + maxCurveWidth(settings, grid, channel);
    if (!pointSetIntersectsArtboard(pointSet, settings, margin)) return [];

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

    if (connected) {
      return nodes.some((node) => node.width > 0)
        ? [variableWidthCurveToPathData(nodes)]
        : [];
    }

    return splitActiveCurveSegments(nodes)
      .filter((segment) => segment.length >= 2)
      .map((segment) => variableWidthCurveToPathData(segment));
  });
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

function repeatDocumentCurvePoints(points, grid, settings, baselineAngle) {
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

    if (pointSetIntersectsArtboard(copy, settings, spacing * 2)) {
      repeated.push(copy);
    }
  }

  return repeated;
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
  const col = clampIndex(Math.floor(point.x / grid.cellWidth), grid.cols);
  const row = clampIndex(Math.floor(point.y / grid.cellHeight), grid.rows);

  const value = sampleChannelValue({
    key,
    sample: samples[row * grid.cols + col],
    settings,
    channel,
  });

  if (value <= 0) {
    return 0;
  }

  const metrics = getMarkMetrics({ value, col, row, grid, settings, channel });
  return metrics.size;
}

function normalizeCurveLayout(layout) {
  if (layout === "document-width") return "full-width";
  if (layout === "document-height") return "full-height";
  if (layout === "document-fit") return "full-width";
  if (layout === "cell-chain") return "tiled-width";
  return layout || "full-width";
}

function usesCurvePatternLayout(layout) {
  return layout === "motif-pattern";
}

function getDocumentCurveNodeCount(layout, grid, settings, channel) {
  const cellSize = Math.max(1, Math.min(grid.cellWidth, grid.cellHeight));
  const quality = normalizeOutputQuality(channel);

  if (layout.startsWith("tiled")) {
    return Math.max(4, Math.ceil((settings.curveTileCells || 12) * 2 * quality));
  }

  const dimension = layout.endsWith("height")
    ? settings.outputHeight
    : settings.outputWidth;
  return Math.max(2, Math.ceil((dimension / cellSize) * quality));
}

function getCurveBaselineAngle(layout, channelRotation) {
  const dimensionAngle = layout.endsWith("height") ? 90 : 0;
  return channelRotation + dimensionAngle;
}

function buildFullCurveBaseline(localPoints, settings, channel, layout, baselineAngle) {
  const targetLength =
    artboardProjectionSpan(settings, baselineAngle) * documentCurveScaleFactor(channel);
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

function buildTiledCurveBaseline(localPoints, grid, settings, channel, layout, baselineAngle) {
  const cellSize = Math.max(1, Math.min(grid.cellWidth, grid.cellHeight));
  const tileLength =
    Math.max(cellSize, (settings.curveTileCells || 12) * cellSize) *
    documentCurveScaleFactor(channel);
  const tile = curveEndpointsConnected(settings, channel)
    ? scaleCurveToLengthByBounds(localPoints, tileLength)
    : scaleCurveForEndpointTiling(localPoints, tileLength);
  const advance = curveEndpointsConnected(settings, channel)
    ? { x: tileLength, y: 0 }
    : tile.at(-1) ?? { x: tileLength, y: 0 };
  const advanceLength = Math.max(1, Math.hypot(advance.x, advance.y));
  const coverLength = artboardProjectionSpan(settings, baselineAngle) + artboardPadding(settings) + tileLength * 4;
  const tileCount = Math.ceil(coverLength / advanceLength);
  const points = [];
  let anchor = { x: 0, y: 0 };

  for (let tileIndex = 0; tileIndex <= tileCount; tileIndex += 1) {
    for (let pointIndex = 0; pointIndex < tile.length; pointIndex += 1) {
      if (tileIndex > 0 && pointIndex === 0) continue;
      const point = tile[pointIndex];
      points.push({
        x: anchor.x + point.x,
        y: anchor.y + point.y,
      });
    }
    anchor = {
      x: anchor.x + advance.x,
      y: anchor.y + advance.y,
    };
  }

  const bounds = getPointBounds(points);
  const centered = points.map((point) => ({
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

function buildCurvePatternStacks(localPoints, settings, channel) {
  const tileCount = normalizeInteger(channel.tileCount, 1, 10000, 1);
  const stackCount = normalizeInteger(channel.stackCount, 1, 10000, 1);
  const tileSpacing = normalizeFinite(channel.tileSpacing, 36);
  const stackSpacing = normalizeFinite(channel.stackSpacing, 36);
  const markRotation = normalizeFinite(channel.rotation, 0);
  const tileDirection = unitVector(normalizeFinite(channel.tileAngle, 0));
  const stackDirection = unitVector(normalizeFinite(channel.stackAngle, 90));
  const tileVector = scaleVector(tileDirection, tileSpacing);
  const stackVector = scaleVector(stackDirection, stackSpacing);
  const tileOffset = normalizeFinite(channel.tileOffset, 0);
  const stackOffset = normalizeFinite(channel.stackOffset, 0);
  const alternateStackOffset = normalizeFinite(channel.alternateStackOffset, 0);
  const tileOrigin = (tileCount - 1) / 2;
  const stackOrigin = (stackCount - 1) / 2;
  const center = {
    x: settings.outputWidth / 2 + normalizeFinite(channel.offsetX, 0),
    y: settings.outputHeight / 2 + normalizeFinite(channel.offsetY, 0),
  };
  const stacks = [];

  for (let stackIndex = 0; stackIndex < stackCount; stackIndex += 1) {
    const stack = [];
    for (let tileIndex = 0; tileIndex < tileCount; tileIndex += 1) {
      const alternate = (tileIndex + stackIndex) % 2 === 1;
      const instanceCenter = {
        x:
          center.x +
          (tileIndex - tileOrigin) * tileVector.x +
          (stackIndex - stackOrigin) * stackVector.x +
          tileDirection.x * tileOffset +
          stackDirection.x * stackOffset +
          (stackIndex % 2 === 1 ? tileDirection.x * alternateStackOffset : 0),
        y:
          center.y +
          (tileIndex - tileOrigin) * tileVector.y +
          (stackIndex - stackOrigin) * stackVector.y +
          tileDirection.y * tileOffset +
          stackDirection.y * stackOffset +
          (stackIndex % 2 === 1 ? tileDirection.y * alternateStackOffset : 0),
      };

      stack.push(
        localPoints.map((point) => {
          const motifPoint = transformMotifPoint(point, instanceCenter, {
            rotation: markRotation,
            flip: alternate && channel.alternateTileTransform === "flip",
            rotate180: alternate && channel.alternateTileTransform === "rotate-180",
          });
          return applyChannelGridTransform(motifPoint, settings, channel);
        }),
      );
    }
    stacks.push(stack);
  }

  return stacks;
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

function transformMotifPoint(point, center, options) {
  const source = {
    x: options.flip ? -point.x : point.x,
    y: point.y,
  };
  const rotatedAlternate = options.rotate180
    ? { x: -source.x, y: -source.y }
    : source;
  const rotated = rotateVector(rotatedAlternate, options.rotation);

  return {
    x: center.x + rotated.x,
    y: center.y + rotated.y,
  };
}

function maxCurveWidth(settings, grid, channel) {
  const cellSize = Math.min(grid.cellWidth, grid.cellHeight);
  const min = Math.max(0, settings.minMark / 100);
  const max = Math.max(min, settings.maxMark / 100) * (channel.maxSize / 100);
  return cellSize * (min + (max - min)) * channel.scale;
}

function documentCurveScaleFactor(channel) {
  const curveScale = channel.curveScale;
  return Number.isFinite(curveScale) && curveScale > 0 ? curveScale / 32 : 1;
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

function artboardPadding(settings) {
  return Math.hypot(settings.outputWidth, settings.outputHeight);
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

  const commands = closed
    ? [
        smoothBoundaryPath(left, { closed: true }),
        smoothBoundaryPath([...right].reverse(), { closed: true }),
      ]
    : [
        smoothBoundaryPath(left),
        roundCapPath(left.at(-1), right.at(-1), sourcePoints.at(-1)),
        smoothBoundaryPath([...right].reverse(), { skipMove: true }),
        roundCapPath(right[0], left[0], sourcePoints[0]),
        "Z",
      ];

  return commands.join(" ");
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
    const p0 = closed
      ? points[(index - 1 + points.length) % points.length]
      : points[Math.max(0, index - 1)];
    const p1 = points[index];
    const p2 = points[(index + 1) % points.length];
    const p3 = closed
      ? points[(index + 2) % points.length]
      : points[Math.min(points.length - 1, index + 2)];
    const c1 = {
      x: p1.x + (p2.x - p0.x) / 6,
      y: p1.y + (p2.y - p0.y) / 6,
    };
    const c2 = {
      x: p2.x - (p3.x - p1.x) / 6,
      y: p2.y - (p3.y - p1.y) / 6,
    };
    commands.push(`C ${round(c1.x)} ${round(c1.y)} ${round(c2.x)} ${round(c2.y)} ${round(p2.x)} ${round(p2.y)}`);
  }

  if (closed) {
    commands.push("Z");
  }

  return commands.join(" ");
}

function roundCapPath(from, to, center) {
  if (!from || !to || !center) return "";
  return `Q ${round(center.x)} ${round(center.y)} ${round(to.x)} ${round(to.y)}`;
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
  return Number.parseFloat(Number(value).toFixed(4));
}

function escapeAttr(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll('"', "&quot;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}
