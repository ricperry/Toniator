import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { aspectLockedDimensions } from "../src/aspect.js";
import { normalizeCurvePathForEditor } from "../src/curveEditor.js";
import { bezziatorDocumentToPathData } from "../src/curveImport.js";
import { pathDataWithEndpointSettings } from "../src/curvePath.js";
import { generateHalftoneSvg } from "../src/svgGenerator.js";

const outputWidth = 360;
const outputHeight = 240;
const baseGrid = {
  cols: 36,
  rows: 24,
  cellWidth: outputWidth / 36,
  cellHeight: outputHeight / 24,
};

const baseSettings = {
  outputWidth,
  outputHeight,
  longEdgeCells: 36,
  gridScale: 92,
  minMark: 0,
  maxMark: 85,
  valueMode: "luminance",
  singleChannel: "k",
  markMode: "curve",
  curveSpan: "full-width",
  curveTileCells: 8,
  useSharedMark: true,
  sharedPreset: "line",
  sharedPath: "",
  showBackground: false,
};

const disabled = {
  enabled: false,
  color: "#000000",
  rotation: 0,
  scale: 1,
  resolutionScale: 1,
  threshold: 0,
  maxSize: 100,
  offsetX: 0,
  offsetY: 0,
  opacity: 1,
  connectEndpoints: false,
  smoothSeamTangents: false,
  preset: "circle",
  customPath: "",
};

const enabledK = {
  ...disabled,
  enabled: true,
  color: "#111111",
  preset: "line",
};

const enabledChannels = {
  c: { ...enabledK, color: "#00aeef", rotation: 15 },
  m: { ...enabledK, color: "#ec008c", rotation: 75 },
  y: { ...enabledK, color: "#ffd400", rotation: 0 },
  k: { ...enabledK, color: "#111111", rotation: 45 },
};

const allBlackSampleProvider = ({ grid }) =>
  Array.from({ length: grid.cols * grid.rows }, () => [0, 0, 0, 255]);

const midGraySampleProvider = ({ grid }) =>
  Array.from({ length: grid.cols * grid.rows }, () => [128, 128, 128, 255]);

const bezziatorFixturePath = "/home/ricperry1/Downloads/bezziator(2).bezvg";

const fallbackBezziatorFixture = JSON.stringify({
  format: "bezziator-working-document",
  version: 1,
  document: {
    paths: [
      {
        id: "path-4",
        nodes: [
          {
            id: "node-1",
            position: { x: 220, y: 420 },
            handleIn: { x: 145.31423744542735, y: -180.9503312946786 },
            handleOut: { x: 161.6032645625147, y: 52.76727094267703 },
            handleInBehavior: "free",
            handleOutBehavior: "free",
            pairConstraint: "independent",
          },
          {
            id: "node-2",
            position: { x: 420, y: 240 },
            handleIn: { x: -120, y: 0 },
            handleOut: { x: 120, y: 0 },
            handleInBehavior: "free",
            handleOutBehavior: "free",
            pairConstraint: "smooth",
          },
          {
            id: "node-3",
            position: { x: 660, y: 420 },
            handleIn: { x: -44.18128848055328, y: 164.15850190653586 },
            handleOut: { x: 52.06671231992744, y: -193.45731615319397 },
            handleInBehavior: "free",
            handleOutBehavior: "free",
            pairConstraint: "smooth",
          },
          {
            id: "node-10",
            position: { x: 980, y: 419 },
            handleIn: { x: -170.051213499382, y: -55.5257251834893 },
            handleOut: { x: 161.60326456251474, y: 52.76727094267704 },
            handleInBehavior: "free",
            handleOutBehavior: "free",
            pairConstraint: "smooth",
          },
        ],
        closed: false,
        fill: "transparent",
        stroke: "#dbe5ff",
        strokeWidth: 2,
      },
    ],
    page: { width: 1024, height: 768 },
  },
});

const loadBezziatorFixture = () =>
  existsSync(bezziatorFixturePath)
    ? readFileSync(bezziatorFixturePath, "utf8")
    : fallbackBezziatorFixture;

const run = ({ settings = {}, channel = {}, channels = null, sampleProvider = allBlackSampleProvider } = {}) =>
  generateHalftoneSvg({
    source: null,
    grid: baseGrid,
    settings: { ...baseSettings, ...settings },
    channels: channels ?? {
      c: disabled,
      m: disabled,
      y: disabled,
      k: { ...enabledK, ...channel },
    },
    sampleProvider,
  });

const assertValidSvg = (svg, label) => {
  assert.match(svg, /^<svg /, `${label}: missing svg root`);
  assert.doesNotMatch(svg, /NaN|Infinity|-Infinity|undefined|null/, `${label}: invalid numeric output`);
  assert.match(svg, /id="halftone-black"/, `${label}: missing black channel group`);
};

const pathCount = (svg) => (svg.match(/<path\b/g) ?? []).length;

const pathCoordinateBounds = (svg) => {
  const numbers = [...svg.matchAll(/[-+]?(?:\d*\.)?\d+(?:e[-+]?\d+)?/gi)]
    .map((match) => Number(match[0]))
    .filter(Number.isFinite);
  const bounds = {
    minX: Infinity,
    maxX: -Infinity,
    minY: Infinity,
    maxY: -Infinity,
  };

  for (let index = 0; index + 1 < numbers.length; index += 2) {
    const x = numbers[index];
    const y = numbers[index + 1];
    bounds.minX = Math.min(bounds.minX, x);
    bounds.maxX = Math.max(bounds.maxX, x);
    bounds.minY = Math.min(bounds.minY, y);
    bounds.maxY = Math.max(bounds.maxY, y);
  }

  return bounds;
};

const generatedPathCoordinateBounds = (svg) => {
  const numbers = [...svg.matchAll(/<path\b[^>]*\sd="([^"]+)"/g)]
    .flatMap((match) => [...match[1].matchAll(/[-+]?(?:\d*\.)?\d+(?:e[-+]?\d+)?/gi)])
    .map((match) => Number(match[0]))
    .filter(Number.isFinite);
  const bounds = {
    minX: Infinity,
    maxX: -Infinity,
    minY: Infinity,
    maxY: -Infinity,
  };

  for (let index = 0; index + 1 < numbers.length; index += 2) {
    const x = numbers[index];
    const y = numbers[index + 1];
    bounds.minX = Math.min(bounds.minX, x);
    bounds.maxX = Math.max(bounds.maxX, x);
    bounds.minY = Math.min(bounds.minY, y);
    bounds.maxY = Math.max(bounds.maxY, y);
  }

  return bounds;
};

const firstGeneratedTransform = (svg) =>
  svg.match(/<path\b[^>]*\stransform="([^"]+)"/)?.[1] ?? "";

const firstGeneratedPathData = (svg) =>
  svg.match(/<path\b[^>]*\sd="([^"]+)"/)?.[1] ?? "";

const assertCurveCoversArtboard = (svg, label) => {
  const bounds = pathCoordinateBounds(svg);
  const tolerance = Math.min(baseGrid.cellWidth, baseGrid.cellHeight) * 2;

  assert.ok(bounds.minX <= tolerance, `${label}: curve does not reach left side (${bounds.minX})`);
  assert.ok(bounds.maxX >= outputWidth - tolerance, `${label}: curve does not reach right side (${bounds.maxX})`);
  assert.ok(bounds.minY <= tolerance, `${label}: curve does not reach top side (${bounds.minY})`);
  assert.ok(bounds.maxY >= outputHeight - tolerance, `${label}: curve does not reach bottom side (${bounds.maxY})`);
};

const assertCurveOutputIsFilledGeometry = (svg, label) => {
  assert.doesNotMatch(svg, /stroke-width=/, `${label}: curve output must not emit production stroke-width`);
  assert.match(svg, /stroke="none"/, `${label}: curve output should be filled outline geometry`);
};

const testCurveGeometrySpansArtboardAndClips = () => {
  const svg = run({
    settings: {
      markMode: "curve",
      curveSpan: "full-width",
      sharedPath: "M -0.45 0 L 0.45 0",
    },
    channel: { rotation: 0 },
  });
  const bounds = generatedPathCoordinateBounds(svg);
  const tolerance = Math.min(baseGrid.cellWidth, baseGrid.cellHeight) * 2;

  assert.match(svg, /<clipPath id="toniator-artboard-clip"/, "curve output should define an artboard clip");
  assert.match(svg, /clip-path="url\(#toniator-artboard-clip\)"/, "curve channel should be clipped to artboard");
  assert.ok(bounds.minX <= tolerance, `curve geometry should start at the artboard left edge (${bounds.minX})`);
  assert.ok(bounds.maxX >= outputWidth - tolerance, `curve geometry should reach the artboard right edge (${bounds.maxX})`);
};

const testCurveRotations = () => {
  const layouts = ["full-width", "full-height", "tiled-width", "tiled-height"];
  const presets = {
    "full-width": "M -0.45 0 L 0.45 0",
    "full-height": "M -0.45 0 L 0.45 0",
    "tiled-width": "M -0.5 0 C -0.25 -0.25 0.25 0.25 0.5 0",
    "tiled-height": "M -0.5 0 C -0.25 -0.25 0.25 0.25 0.5 0",
  };

  for (const layout of layouts) {
    for (let rotation = 0; rotation <= 180; rotation += 15) {
      const label = `curve ${layout} rotation ${rotation}`;
      const svg = run({
        settings: {
          markMode: "curve",
          curveSpan: layout,
          sharedPath: presets[layout],
        },
        channel: { rotation },
      });

      assertValidSvg(svg, label);
      assert.ok(pathCount(svg) > 0, `${label}: expected curve paths`);
      assertCurveCoversArtboard(svg, label);
      assertCurveOutputIsFilledGeometry(svg, label);
    }
  }
};

const testAllChannelCurveRotations = () => {
  for (const layout of ["full-width", "tiled-width"]) {
    for (let rotation = 0; rotation <= 180; rotation += 15) {
      const label = `all-channel curve ${layout} rotation ${rotation}`;
      const svg = run({
        settings: {
          markMode: "curve",
          curveSpan: layout,
          sharedPath: layout === "tiled-width"
            ? "M -0.5 0 C -0.25 -0.25 0.25 0.25 0.5 0"
            : "M -0.45 0 L 0.45 0",
        },
        channels: Object.fromEntries(
          Object.entries(enabledChannels).map(([key, channel]) => [
            key,
            { ...channel, rotation },
          ]),
        ),
      });

      assertValidSvg(svg, label);
      for (const id of ["cyan", "magenta", "yellow", "black"]) {
        assert.match(svg, new RegExp(`id="halftone-${id}"`), `${label}: missing ${id}`);
      }
      assertCurveCoversArtboard(svg, label);
      assertCurveOutputIsFilledGeometry(svg, label);
    }
  }
};

const testBezziatorFixtureImport = () => {
  const pathData = bezziatorDocumentToPathData(loadBezziatorFixture());
  assert.match(pathData, /^M 220 420 C /, "Bezziator fixture should import as an open cubic path");
  assert.doesNotMatch(pathData, /NaN|Infinity|-Infinity|undefined|null/, "Bezziator import should emit valid numbers");
  assert.equal((pathData.match(/\bC\b/g) ?? []).length, 3, "Bezziator fixture should retain all cubic segments");

  const cubicNumbers = [...pathData.matchAll(/[-+]?(?:\d*\.)?\d+(?:e[-+]?\d+)?/gi)].map((match) => Number(match[0]));
  const node2 = { x: cubicNumbers[6], y: cubicNumbers[7] };
  const node2In = {
    x: cubicNumbers[4] - node2.x,
    y: cubicNumbers[5] - node2.y,
  };
  const node2Out = {
    x: cubicNumbers[8] - node2.x,
    y: cubicNumbers[9] - node2.y,
  };
  const node3 = { x: cubicNumbers[12], y: cubicNumbers[13] };
  const node3In = {
    x: cubicNumbers[10] - node3.x,
    y: cubicNumbers[11] - node3.y,
  };
  const node3Out = {
    x: cubicNumbers[14] - node3.x,
    y: cubicNumbers[15] - node3.y,
  };

  assertSmoothPair(node2In, node2Out, "node-2");
  assertSmoothPair(node3In, node3Out, "node-3");
};

const testEndpointSettings = () => {
  const open = "M 0 0 C 20 0 80 0 100 0";
  const connected = pathDataWithEndpointSettings(open, {
    connectEndpoints: true,
    smoothSeamTangents: false,
  });
  const smooth = pathDataWithEndpointSettings(open, {
    connectEndpoints: true,
    smoothSeamTangents: true,
  });

  assert.equal((open.match(/\bC\b/g) ?? []).length, 1, "fixture sanity");
  assert.equal((connected.match(/\bC\b/g) ?? []).length, 2, "connected curve should add an explicit seam segment");
  assert.equal((smooth.match(/\bC\b/g) ?? []).length, 2, "smooth seam should keep the explicit seam segment");

  const nums = [...smooth.matchAll(/[-+]?(?:\d*\.)?\d+(?:e[-+]?\d+)?/gi)].map((match) => Number(match[0]));
  const startOut = { x: nums[2] - nums[0], y: nums[3] - nums[1] };
  const endIn = { x: nums[4] - nums[6], y: nums[5] - nums[7] };
  assertSmoothPair(endIn, startOut, "smooth endpoint seam");
};

const testConnectedCurveOutput = () => {
  for (const smoothCurveSeam of [false, true]) {
    const label = `connected curve output smooth=${smoothCurveSeam}`;
    const svg = run({
      settings: {
        markMode: "curve",
        curveSpan: "full-width",
        sharedPath: "M -0.45 0 C -0.2 -0.35 0.2 0.35 0.45 0",
      },
      channel: {
        connectEndpoints: true,
        smoothSeamTangents: smoothCurveSeam,
      },
    });

    assertValidSvg(svg, label);
    assert.ok(pathCount(svg) > 0, `${label}: expected generated curve paths`);
    assertCurveOutputIsFilledGeometry(svg, label);
    assert.match(svg, /fill-rule="nonzero"/, `${label}: connected curve should use compound channel fill`);
  }
};

const testCurvePatternTilingAndStacking = () => {
  const basePattern = run({
    settings: {
      markMode: "curve",
      curveSpan: "motif-pattern",
      sharedPath: "M -0.5 0 L 0.5 0",
    },
    channel: {
      rotation: 0,
      curveScale: 24,
      tileCount: 3,
      tileSpacing: 30,
      tileAngle: 0,
      tileOffset: 0,
      stackCount: 2,
      stackSpacing: 40,
      stackAngle: 90,
      stackOffset: 0,
      alternateStackOffset: 15,
      alternateTileTransform: "flip",
      outputQuality: 1,
    },
  });
  const highQualityPattern = run({
    settings: {
      markMode: "curve",
      curveSpan: "motif-pattern",
      sharedPath: "M -0.5 0 L 0.5 0",
    },
    channel: {
      rotation: 0,
      curveScale: 24,
      tileCount: 3,
      tileSpacing: 30,
      stackCount: 2,
      stackSpacing: 40,
      outputQuality: 3,
    },
  });
  const moreTilesPattern = run({
    settings: {
      markMode: "curve",
      curveSpan: "motif-pattern",
      sharedPath: "M -0.5 0 L 0.5 0",
    },
    channel: {
      rotation: 0,
      curveScale: 24,
      tileCount: 4,
      tileSpacing: 30,
      stackCount: 2,
      stackSpacing: 40,
      outputQuality: 1,
    },
  });
  const connectedPattern = run({
    settings: {
      markMode: "curve",
      curveSpan: "motif-pattern",
      sharedPath: "M -0.5 0 L 0.5 0",
    },
    channel: {
      rotation: 0,
      curveScale: 24,
      tileCount: 3,
      tileSpacing: 30,
      stackCount: 2,
      stackSpacing: 40,
      connectEndpoints: true,
      smoothSeamTangents: true,
      outputQuality: 1,
    },
  });

  assertValidSvg(basePattern, "curve pattern tiling/stacking");
  assertCurveOutputIsFilledGeometry(basePattern, "curve pattern tiling/stacking");
  assert.equal(pathCount(basePattern), 1, "curve pattern should emit one compound path per channel");
  assert.equal(
    pathCount(highQualityPattern),
    pathCount(basePattern),
    "output quality must not change compound channel path count",
  );
  assert.equal(
    pathCount(moreTilesPattern),
    1,
    "tile count should still emit one compound path per channel",
  );
  assert.equal(
    pathCount(connectedPattern),
    1,
    "connected motif tiles should emit one compound path per channel",
  );
  assert.doesNotMatch(
    connectedPattern,
    /fill-rule="evenodd"/,
    "connected motif tiles should not close individual source curves",
  );
};

const testCurveLayoutSelectionOverridesMotifControls = () => {
  const sharedPath = "M -0.5 0 C -0.2 -0.28 0.2 0.28 0.5 0";
  const motifControls = {
    tileCount: 9,
    tileSpacing: 22,
    tileAngle: 18,
    tileOffset: 7,
    stackCount: 5,
    stackSpacing: 28,
    stackAngle: 78,
    stackOffset: -4,
    alternateStackOffset: 13,
    alternateTileTransform: "flip",
  };
  const fullWidthBase = run({
    settings: { markMode: "curve", curveSpan: "full-width", sharedPath },
    channel: {},
  });
  const fullWidthWithMotifControls = run({
    settings: { markMode: "curve", curveSpan: "full-width", sharedPath },
    channel: motifControls,
  });
  const motifPattern = run({
    settings: { markMode: "curve", curveSpan: "motif-pattern", sharedPath },
    channel: motifControls,
  });
  const fullHeight = run({
    settings: { markMode: "curve", curveSpan: "full-height", sharedPath },
    channel: motifControls,
  });
  const cellChain = run({
    settings: { markMode: "curve", curveSpan: "cell-chain", sharedPath, curveTileCells: 4 },
    channel: motifControls,
  });

  assert.equal(
    firstGeneratedPathData(fullWidthWithMotifControls),
    firstGeneratedPathData(fullWidthBase),
    "full-width layout should ignore motif tile/stack controls",
  );
  assert.notEqual(
    firstGeneratedPathData(motifPattern),
    firstGeneratedPathData(fullWidthBase),
    "motif-pattern layout should use motif tile/stack controls",
  );
  assert.notEqual(
    firstGeneratedPathData(fullHeight),
    firstGeneratedPathData(fullWidthBase),
    "full-height layout should differ from full-width",
  );
  assert.notEqual(
    firstGeneratedPathData(cellChain),
    firstGeneratedPathData(fullWidthBase),
    "connected cell chains should use tiled-width chain behavior",
  );
};

const testBezziatorFixtureCurveRotations = () => {
  const pathData = bezziatorDocumentToPathData(loadBezziatorFixture());

  for (const layout of ["full-width", "full-height", "tiled-width", "tiled-height"]) {
    for (let rotation = 0; rotation <= 180; rotation += 15) {
      const label = `Bezziator fixture ${layout} rotation ${rotation}`;
      const svg = run({
        settings: {
          markMode: "curve",
          curveSpan: layout,
          sharedPath: pathData,
          curveTileCells: 12,
        },
        channel: { rotation },
      });

      assertValidSvg(svg, label);
      assert.ok(pathCount(svg) > 0, `${label}: expected generated curve paths`);
      assertCurveCoversArtboard(svg, label);
      assertCurveOutputIsFilledGeometry(svg, label);
    }
  }
};

const testShapeRotationsAndRanges = () => {
  for (let rotation = 0; rotation <= 180; rotation += 15) {
    const label = `shape rotation ${rotation}`;
    const svg = run({
      settings: {
        markMode: "shape",
        sharedPreset: "circle",
        sharedPath: "",
      },
      channel: { rotation },
    });

    assertValidSvg(svg, label);
    assert.ok(pathCount(svg) > 0, `${label}: expected shape paths`);
  }

  for (let threshold = 0; threshold <= 100; threshold += 1) {
    const label = `threshold ${threshold}`;
    const svg = run({
      settings: { markMode: "shape", sharedPreset: "circle", sharedPath: "" },
      channel: { threshold: threshold / 100 },
    });
    assertValidSvg(svg, label);
  }

  for (let maxSize = 0; maxSize <= 100; maxSize += 1) {
    const label = `max size ${maxSize}`;
    const svg = run({
      settings: { markMode: "shape", sharedPreset: "circle", sharedPath: "" },
      channel: { maxSize },
    });
    assertValidSvg(svg, label);
  }

  for (let opacity = 0; opacity <= 100; opacity += 1) {
    const label = `opacity ${opacity}`;
    const svg = run({
      settings: { markMode: "shape", sharedPreset: "circle", sharedPath: "" },
      channel: { opacity: opacity / 100 },
    });
    assertValidSvg(svg, label);
    assert.match(svg, new RegExp(`opacity="${Number.parseFloat((opacity / 100).toFixed(4))}"`), `${label}: opacity not reflected`);
  }
};

const testShapeGridRotationAndPivot = () => {
  const base = run({
    settings: { markMode: "shape", sharedPreset: "circle", sharedPath: "" },
    channel: { rotation: 0, gridRotation: 0, gridPivotX: 0, gridPivotY: 0 },
  });
  const gridRotated = run({
    settings: { markMode: "shape", sharedPreset: "circle", sharedPath: "" },
    channel: { rotation: 0, gridRotation: 33, gridPivotX: 0, gridPivotY: 0 },
  });
  const pivotRotated = run({
    settings: { markMode: "shape", sharedPreset: "circle", sharedPath: "" },
    channel: { rotation: 0, gridRotation: 33, gridPivotX: 42, gridPivotY: -27 },
  });

  assertValidSvg(base, "shape grid rotation base");
  assertValidSvg(gridRotated, "shape grid rotation");
  assertValidSvg(pivotRotated, "shape grid rotation pivot");
  assert.notEqual(
    firstGeneratedPathData(gridRotated),
    firstGeneratedPathData(base),
    "rotating the channel grid should move circle mark placement even when mark rotation is unchanged",
  );
  assert.notEqual(
    firstGeneratedPathData(pivotRotated),
    firstGeneratedPathData(gridRotated),
    "changing the channel grid pivot should change rotated grid placement",
  );
  assert.doesNotMatch(
    gridRotated,
    /<path\b[^>]*\stransform=/,
    "shape transforms should be baked into the compound channel path",
  );
};

const testAllShapePresets = () => {
  for (const preset of ["circle", "rectangle", "triangle", "pentagon", "hexagon"]) {
    const label = `shape preset ${preset}`;
    const svg = run({
      settings: {
        markMode: "shape",
        geometryMode: "per-channel",
        useSharedMark: false,
      },
      channels: {
        c: { ...disabled },
        m: { ...disabled },
        y: { ...disabled },
        k: { ...enabledK, preset },
      },
    });

    assertValidSvg(svg, label);
    assert.ok(pathCount(svg) > 0, `${label}: expected generated paths`);
  }
};

const testFullInputConfigurationSurface = () => {
  const curvePath = "M -0.5 0 C -0.2 -0.35 0.2 0.35 0.5 0";
  const cases = [
    {
      label: "shape independent channels full controls",
      settings: {
        outputWidth: 420,
        outputHeight: 260,
        longEdgeCells: 28,
        gridScale: 37,
        minMark: 12,
        maxMark: 94,
        valueMode: "cmyk",
        singleChannel: "m",
        markMode: "shape",
        geometryMode: "per-channel",
        useSharedMark: false,
        sharedPreset: "circle",
        sharedPath: "",
        showBackground: true,
      },
      channels: {
        c: {
          ...enabledChannels.c,
          preset: "triangle",
          rotation: 0,
          scale: 0.65,
          resolutionScale: 0.5,
          threshold: 0,
          maxSize: 28,
          offsetX: -13.25,
          offsetY: 8.5,
          opacity: 0.34,
        },
        m: {
          ...enabledChannels.m,
          preset: "pentagon",
          rotation: 61,
          scale: 1.8,
          resolutionScale: 1.25,
          threshold: 0.25,
          maxSize: 72,
          offsetX: 6.5,
          offsetY: -11.5,
          opacity: 0.67,
        },
        y: {
          ...enabledChannels.y,
          preset: "hexagon",
          rotation: 122,
          scale: 0.95,
          resolutionScale: 1.75,
          threshold: 0.5,
          maxSize: 100,
          offsetX: 19,
          offsetY: 3,
          opacity: 1,
        },
        k: {
          ...enabledChannels.k,
          preset: "rectangle",
          rotation: 180,
          scale: 1.1,
          resolutionScale: 0.25,
          threshold: 0.75,
          maxSize: 140,
          offsetX: -4,
          offsetY: 17,
          opacity: 0.9,
        },
      },
    },
    {
      label: "curve synchronized full controls",
      settings: {
        outputWidth: 300,
        outputHeight: 360,
        longEdgeCells: 32,
        gridScale: 88,
        minMark: 4,
        maxMark: 100,
        valueMode: "single-channel",
        singleChannel: "k",
        markMode: "curve",
        curveSpan: "tiled-width",
        curveTileCells: 5,
        syncCurveChannels: true,
        sharedConnectEndpoints: true,
        sharedSmoothSeamTangents: true,
        sharedPreset: "wave",
        sharedPath: curvePath,
        showBackground: true,
      },
      channels: {
        c: {
          ...enabledChannels.c,
          rotation: 0,
          scale: 0.75,
          resolutionScale: 0.4,
          threshold: 0,
          maxSize: 45,
          offsetX: -7,
          offsetY: 3,
          opacity: 0.5,
          customPath: "M -0.5 0 L 0.5 0",
          connectEndpoints: false,
          smoothSeamTangents: false,
        },
        m: {
          ...enabledChannels.m,
          rotation: 45,
          scale: 1.1,
          resolutionScale: 1,
          threshold: 0.2,
          maxSize: 75,
          offsetX: 4,
          offsetY: -2,
          opacity: 0.65,
          customPath: "M -0.5 -0.1 L 0.5 0.1",
          connectEndpoints: true,
          smoothSeamTangents: false,
        },
        y: {
          ...enabledChannels.y,
          rotation: 90,
          scale: 1.5,
          resolutionScale: 1.6,
          threshold: 0.4,
          maxSize: 100,
          offsetX: 11,
          offsetY: 9,
          opacity: 0.8,
          customPath: "M -0.5 0 C 0 -0.25 0 0.25 0.5 0",
          connectEndpoints: false,
          smoothSeamTangents: true,
        },
        k: {
          ...enabledChannels.k,
          rotation: 135,
          scale: 1.9,
          resolutionScale: 2,
          threshold: 0.6,
          maxSize: 125,
          offsetX: -5,
          offsetY: -10,
          opacity: 1,
          customPath: "M -0.5 0 C -0.25 0.2 0.25 -0.2 0.5 0",
          connectEndpoints: true,
          smoothSeamTangents: true,
        },
      },
    },
    {
      label: "curve independent channel paths and continuity",
      settings: {
        outputWidth: 500,
        outputHeight: 220,
        longEdgeCells: 30,
        gridScale: 54,
        minMark: 0,
        maxMark: 83,
        valueMode: "rgb-inverted",
        singleChannel: "c",
        markMode: "curve",
        curveSpan: "full-height",
        curveTileCells: 9,
        syncCurveChannels: false,
        sharedConnectEndpoints: false,
        sharedSmoothSeamTangents: false,
        sharedPreset: "line",
        sharedPath: "M -0.5 0 L 0.5 0",
        showBackground: false,
      },
      channels: {
        c: {
          ...enabledChannels.c,
          rotation: 15,
          scale: 1,
          resolutionScale: 0.7,
          threshold: 0.1,
          maxSize: 65,
          offsetX: 0,
          offsetY: 0,
          opacity: 0.7,
          customPath: curvePath,
          connectEndpoints: true,
          smoothSeamTangents: true,
        },
        m: {
          ...enabledChannels.m,
          enabled: false,
          customPath: "M -0.5 0 L 0.5 0",
        },
        y: {
          ...enabledChannels.y,
          rotation: 120,
          scale: 0.8,
          resolutionScale: 1.3,
          threshold: 0.3,
          maxSize: 88,
          offsetX: 5,
          offsetY: -5,
          opacity: 0.55,
          customPath: "M -0.5 0 L 0 0.25 L 0.5 0",
          connectEndpoints: false,
          smoothSeamTangents: false,
        },
        k: {
          ...enabledChannels.k,
          rotation: 170,
          scale: 1.4,
          resolutionScale: 1.9,
          threshold: 0.55,
          maxSize: 110,
          offsetX: -9,
          offsetY: 6,
          opacity: 0.95,
          customPath: "M -0.5 0 C -0.1 -0.4 0.1 0.4 0.5 0",
          connectEndpoints: true,
          smoothSeamTangents: false,
        },
      },
    },
  ];

  for (const testCase of cases) {
    const rows = Math.max(
      2,
      Math.round(
        testCase.settings.longEdgeCells *
          (testCase.settings.outputHeight / testCase.settings.outputWidth),
      ),
    );
    const svg = generateHalftoneSvg({
      source: null,
      grid: {
        cols: testCase.settings.longEdgeCells,
        rows,
        cellWidth: testCase.settings.outputWidth / testCase.settings.longEdgeCells,
        cellHeight: testCase.settings.outputHeight / rows,
      },
      settings: { ...baseSettings, ...testCase.settings },
      channels: testCase.channels,
      includePreviewBackground: testCase.settings.showBackground,
      sampleProvider: allBlackSampleProvider,
    });

    assert.match(svg, /^<svg /, `${testCase.label}: missing svg root`);
    assert.doesNotMatch(svg, /NaN|Infinity|-Infinity|undefined|null/, `${testCase.label}: invalid output`);
    assert.ok(pathCount(svg) > 0, `${testCase.label}: expected paths`);
  }
};

const testThresholdBoundaryWithMidGray = () => {
  const belowOrAtMid = run({
    settings: { markMode: "shape", sharedPreset: "circle", sharedPath: "" },
    channel: { threshold: 0.49 },
    sampleProvider: midGraySampleProvider,
  });
  const aboveMid = run({
    settings: { markMode: "shape", sharedPreset: "circle", sharedPath: "" },
    channel: { threshold: 0.51 },
    sampleProvider: midGraySampleProvider,
  });

  assert.ok(pathCount(belowOrAtMid) > pathCount(aboveMid), "threshold boundary should omit mid-gray above threshold");
};

const testShapeOffsetPeriodicity = () => {
  const zero = run({
    settings: { markMode: "shape", sharedPreset: "circle", sharedPath: "" },
  });
  const shifted = run({
    settings: { markMode: "shape", sharedPreset: "circle", sharedPath: "" },
    channel: {
      offsetX: baseGrid.cellWidth * 3,
      offsetY: baseGrid.cellHeight * 2,
    },
  });

  assert.equal(shifted, zero, "shape offsets should be periodic by full grid cells");
};

const testAspectLocking = () => {
  assert.deepEqual(
    aspectLockedDimensions({
      width: 450,
      height: 999,
      sourceWidth: 900,
      sourceHeight: 620,
      editedDimension: "width",
    }),
    { width: 450, height: 310 },
    "editing width should derive height from source aspect",
  );

  assert.deepEqual(
    aspectLockedDimensions({
      width: 999,
      height: 310,
      sourceWidth: 900,
      sourceHeight: 620,
      editedDimension: "height",
    }),
    { width: 450, height: 310 },
    "editing height should derive width from source aspect",
  );
};

const testEditorCurveNormalization = () => {
  const normalized = normalizeCurvePathForEditor(
    "M 40528.507 141756.013 C 35235.288 148081.368 61394.187 145126.477 61394.434 145126.307",
  );
  const numbers = [...normalized.matchAll(/[-+]?(?:\d*\.)?\d+(?:e[-+]?\d+)?/gi)]
    .map((match) => Number(match[0]));

  assert.ok(numbers.length >= 8, "editor normalization should preserve path coordinates");
  assert.ok(
    numbers.every((value) => Number.isFinite(value) && Math.abs(value) <= 1.01),
    `editor normalization should keep local-space coordinates, got ${normalized}`,
  );
  assert.doesNotMatch(normalized, /[zZ]\s*$/, "editor normalization must keep the edited path open");
};

function assertSmoothPair(handleIn, handleOut, label) {
  const cross = handleIn.x * handleOut.y - handleIn.y * handleOut.x;
  const dot = handleIn.x * handleOut.x + handleIn.y * handleOut.y;
  const lengthProduct = Math.hypot(handleIn.x, handleIn.y) * Math.hypot(handleOut.x, handleOut.y);

  assert.ok(Math.abs(cross) <= lengthProduct * 1e-5, `${label}: smooth handles are not collinear`);
  assert.ok(dot < 0, `${label}: smooth handles are not opposed`);
}

testCurveRotations();
testCurveGeometrySpansArtboardAndClips();
testAllChannelCurveRotations();
testBezziatorFixtureImport();
testEndpointSettings();
testConnectedCurveOutput();
testCurvePatternTilingAndStacking();
testCurveLayoutSelectionOverridesMotifControls();
testBezziatorFixtureCurveRotations();
testShapeRotationsAndRanges();
testShapeGridRotationAndPivot();
testAllShapePresets();
testFullInputConfigurationSurface();
testThresholdBoundaryWithMidGray();
testShapeOffsetPeriodicity();
testAspectLocking();
testEditorCurveNormalization();

console.log("generator sweep tests passed");
