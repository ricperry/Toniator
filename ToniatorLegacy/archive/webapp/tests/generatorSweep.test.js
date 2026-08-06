import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { aspectLockedDimensions } from "../src/aspect.js";
import { normalizeCurvePathForEditor } from "../src/curveEditor.js";
import { bezziatorDocumentToPathData } from "../src/curveImport.js";
import { pathDataWithEndpointSettings } from "../src/curvePath.js";
import { mapPixelToChannels } from "../src/color.js";
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

const run = ({
  settings = {},
  channel = {},
  channels = null,
  sampleProvider = allBlackSampleProvider,
  generatorOptions = {},
} = {}) =>
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
    ...generatorOptions,
  });

const assertValidSvg = (svg, label) => {
  assert.match(svg, /^<svg /, `${label}: missing svg root`);
  assert.doesNotMatch(svg, /NaN|Infinity|-Infinity|undefined|null/, `${label}: invalid numeric output`);
  assert.match(svg, /id="toniator-black"/, `${label}: missing black channel group`);
};

const pathCount = (svg) => (svg.match(/<path\b/g) ?? []).length;
const centerlinePathCount = (svg) => (svg.match(/id="black-centerline-\d+"/g) ?? []).length;

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

const commandCount = (pathData, command) =>
  (pathData.match(new RegExp(`\\b${command}\\b`, "g")) ?? []).length;
const smoothCurveCommandCount = (pathData) =>
  commandCount(pathData, "C") + commandCount(pathData, "S");

const assertNoDegenerateQuadraticCaps = (svg, label) => {
  for (const [, pathData] of svg.matchAll(/<path\b[^>]*\sd="([^"]+)"/g)) {
    assert.doesNotMatch(pathData, /\bQ\b/, `${label}: curve outline caps should be cubic, not quadratic`);
    let current = { x: 0, y: 0 };
    let subpathStart = null;
    const tokens = pathData.match(/[A-Z]|[-+]?(?:\d*\.)?\d+(?:e[-+]?\d+)?/g) ?? [];

    for (let index = 0; index < tokens.length;) {
      const command = tokens[index++];
      if (command === "M") {
        current = { x: Number(tokens[index++]), y: Number(tokens[index++]) };
        subpathStart = current;
      } else if (command === "C") {
        index += 4;
        current = { x: Number(tokens[index++]), y: Number(tokens[index++]) };
      } else if (command === "S" || command === "Q") {
        index += 2;
        const end = { x: Number(tokens[index++]), y: Number(tokens[index++]) };
        if (command === "Q") {
          assert.ok(
            Math.hypot(current.x - end.x, current.y - end.y) > 0.001,
            `${label}: degenerate quadratic cap at ${end.x},${end.y}`,
          );
        }
        current = end;
      } else if (command === "L") {
        current = { x: Number(tokens[index++]), y: Number(tokens[index++]) };
      } else if (command === "Z") {
        current = subpathStart ?? current;
      } else {
        break;
      }
    }
  }
};

const assertNodeTypesAligned = (svg, label) => {
  for (const [, tag, pathData] of svg.matchAll(/(<path\b[^>]*\sd="([^"]+)"[^>]*>)/g)) {
    const nodeTypes = tag.match(/sodipodi:nodetypes="([^"]+)"/)?.[1];
    if (!nodeTypes) continue;

    assert.equal(
      nodeTypes.length,
      countSodipodiNodes(pathData),
      `${label}: nodetype count does not match exported path node count`,
    );
  }
};

const countSodipodiNodes = (pathData) => {
  let count = 0;
  let current = { x: 0, y: 0 };
  let subpathStart = null;
  const tokens = pathData.match(/[A-Z]|[-+]?(?:\d*\.)?\d+(?:e[-+]?\d+)?/g) ?? [];

  for (let index = 0; index < tokens.length;) {
    const command = tokens[index++];
    if (command === "M") {
      current = { x: Number(tokens[index++]), y: Number(tokens[index++]) };
      subpathStart = current;
      count += 1;
    } else if (command === "C") {
      index += 4;
      current = { x: Number(tokens[index++]), y: Number(tokens[index++]) };
      count += 1;
    } else if (command === "S" || command === "Q") {
      index += 2;
      current = { x: Number(tokens[index++]), y: Number(tokens[index++]) };
      count += 1;
    } else if (command === "L") {
      current = { x: Number(tokens[index++]), y: Number(tokens[index++]) };
      count += 1;
    } else if (command === "Z") {
      if (
        subpathStart &&
        Math.hypot(current.x - subpathStart.x, current.y - subpathStart.y) <= 0.001
      ) {
        count -= 1;
      }
      current = subpathStart ?? current;
      subpathStart = null;
    } else {
      break;
    }
  }

  return count;
};

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

  for (const layout of ["full-width", "full-height"]) {
    const scaledSvg = run({
      settings: {
        markMode: "curve",
        curveSpan: layout,
        sharedPath: "M -0.45 0 L 0.45 0",
      },
      channel: { rotation: 0, curveScale: 16 },
    });
    const scaledBounds = generatedPathCoordinateBounds(scaledSvg);
    assert.ok(
      scaledBounds.minX <= tolerance,
      `${layout} should ignore source curve scale and reach the left edge (${scaledBounds.minX})`,
    );
    assert.ok(
      scaledBounds.maxX >= outputWidth - tolerance,
      `${layout} should ignore source curve scale and reach the right edge (${scaledBounds.maxX})`,
    );
    assert.ok(
      scaledBounds.minY <= tolerance,
      `${layout} should ignore source curve scale and reach the top edge (${scaledBounds.minY})`,
    );
    assert.ok(
      scaledBounds.maxY >= outputHeight - tolerance,
      `${layout} should ignore source curve scale and reach the bottom edge (${scaledBounds.maxY})`,
    );
  }

  for (const layout of ["full-width", "full-height"]) {
    const rotatedGridSvg = run({
      settings: {
        markMode: "curve",
        curveSpan: layout,
        sharedPath: "M -0.45 0 L 0.45 0",
      },
      channel: { rotation: 0, gridRotation: 30, gridPivotX: 90, gridPivotY: -60 },
    });
    const rotatedBounds = generatedPathCoordinateBounds(rotatedGridSvg);
    assert.ok(
      rotatedBounds.minX <= tolerance,
      `${layout} with rotated grid should reach the left edge (${rotatedBounds.minX})`,
    );
    assert.ok(
      rotatedBounds.maxX >= outputWidth - tolerance,
      `${layout} with rotated grid should reach the right edge (${rotatedBounds.maxX})`,
    );
    assert.ok(
      rotatedBounds.minY <= tolerance,
      `${layout} with rotated grid should reach the top edge (${rotatedBounds.minY})`,
    );
    assert.ok(
      rotatedBounds.maxY >= outputHeight - tolerance,
      `${layout} with rotated grid should reach the bottom edge (${rotatedBounds.maxY})`,
    );
  }
};

const testCurveRotations = () => {
  const layouts = ["full-width", "full-height"];
  const presets = {
    "full-width": "M -0.45 0 L 0.45 0",
    "full-height": "M -0.45 0 L 0.45 0",
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
  for (const layout of ["full-width", "full-height"]) {
    for (let rotation = 0; rotation <= 180; rotation += 15) {
      const label = `all-channel curve ${layout} rotation ${rotation}`;
      const svg = run({
        settings: {
          markMode: "curve",
          curveSpan: layout,
          sharedPath: "M -0.45 0 L 0.45 0",
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
        assert.match(svg, new RegExp(`id="toniator-${id}"`), `${label}: missing ${id}`);
        assert.match(svg, new RegExp(`inkscape:label="${id[0].toUpperCase()}${id.slice(1)}"`), `${label}: missing ${id} layer label`);
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
  const autoCoveragePattern = run({
    settings: {
      markMode: "curve",
      curveSpan: "motif-pattern",
      sharedPath: "M -0.5 0 L 0.5 0",
    },
    channel: {
      rotation: 0,
      curveScale: 18,
      motifCoverageMode: "auto",
      motifBleed: 2,
      tileCount: 1,
      stackCount: 1,
      tileSpacing: 24,
      tileAngle: 16,
      stackSpacing: 20,
      stackAngle: 106,
      outputQuality: 1,
    },
  });
  const autoCoverageBounds = generatedPathCoordinateBounds(autoCoveragePattern);
  const autoCoverageTolerance = Math.min(baseGrid.cellWidth, baseGrid.cellHeight) * 2;
  const defaultFloodPattern = run({
    settings: {
      markMode: "curve",
      curveSpan: "motif-pattern",
      sharedPath: "M -0.5 0 L 0.5 0",
    },
    channel: {
      rotation: 0,
      curveScale: 32,
      motifCoverageMode: "auto",
      motifBleed: 2,
      stackSpacing: 36,
      stackAngle: 0,
      outputQuality: 1,
    },
  });
  const defaultFloodBounds = generatedPathCoordinateBounds(defaultFloodPattern);
  const defaultSpacingPattern = run({
    settings: {
      markMode: "curve",
      curveSpan: "motif-pattern",
      sharedPath: "M -0.5 0 L 0.5 0",
    },
    channel: {
      rotation: 0,
      curveScale: 32,
      motifCoverageMode: "manual",
      tileCount: 4,
      stackCount: 1,
      stackSpacing: 36,
      stackAngle: 0,
      outputQuality: 1,
    },
  });
  const explicitSpacingPattern = run({
    settings: {
      markMode: "curve",
      curveSpan: "motif-pattern",
      sharedPath: "M -0.5 0 L 0.5 0",
    },
    channel: {
      rotation: 0,
      curveScale: 32,
      motifCoverageMode: "manual",
      tileCount: 4,
      tileSpacing: 32,
      stackCount: 1,
      stackSpacing: 36,
      stackAngle: 0,
      outputQuality: 1,
    },
  });
  const noGapRowPattern = run({
    settings: {
      markMode: "curve",
      curveSpan: "motif-pattern",
      sharedPath: "M -0.5 0 L 0.5 0",
    },
    channel: {
      rotation: 0,
      curveScale: 32,
      motifCoverageMode: "manual",
      tileCount: 8,
      tileSpacing: 33,
      stackCount: 1,
      stackSpacing: 36,
      stackAngle: 0,
      outputQuality: 1,
    },
  });
  const noGapPathData = firstGeneratedPathData(noGapRowPattern);
  const alternatingFlipPattern = run({
    settings: {
      markMode: "curve",
      curveSpan: "motif-pattern",
      sharedPath: "M -0.5 0 C -0.25 -0.2 0.25 0.2 0.5 0",
    },
    channel: {
      rotation: 0,
      curveScale: 32,
      motifCoverageMode: "manual",
      tileCount: 30,
      stackCount: 1,
      stackSpacing: 36,
      stackAngle: 0,
      alternateTileTransform: "flip",
      outputQuality: 1,
    },
  });
  const alternatingRotatePattern = run({
    settings: {
      markMode: "curve",
      curveSpan: "motif-pattern",
      sharedPath: "M -0.5 0 C -0.25 -0.2 0.25 0.2 0.5 0",
    },
    channel: {
      rotation: 0,
      curveScale: 32,
      motifCoverageMode: "manual",
      tileCount: 30,
      stackCount: 1,
      stackSpacing: 36,
      stackAngle: 0,
      alternateTileTransform: "rotate-180",
      outputQuality: 1,
    },
  });
  const alternatingFlipBounds = generatedPathCoordinateBounds(alternatingFlipPattern);
  const alternatingRotateBounds = generatedPathCoordinateBounds(alternatingRotatePattern);
  const clippedRotatedPattern = run({
    settings: {
      markMode: "curve",
      curveSpan: "motif-pattern",
      sharedPath: "M -0.5 -0.08 C -0.18 -0.2 0.2 0.2 0.5 0.08",
    },
    channel: {
      rotation: 0,
      curveScale: 32,
      motifCoverageMode: "auto",
      motifBleed: 2,
      gridRotation: 45,
      stackSpacing: 6,
      outputQuality: 3,
    },
  });
  const clippedRotatedBounds = generatedPathCoordinateBounds(clippedRotatedPattern);

  assertValidSvg(basePattern, "curve pattern tiling/stacking");
  assertCurveOutputIsFilledGeometry(basePattern, "curve pattern tiling/stacking");
  assert.match(basePattern, /xmlns:sodipodi=/, "curve export should include Inkscape node type namespace");
  assert.match(
    alternatingFlipPattern,
    /<path id="black-centerline-001"[^>]*sodipodi:nodetypes="c+s+c+s+c+"/,
    "centerline export should mark rail interiors smooth and cap junctions cusp",
  );
  assert.doesNotMatch(firstGeneratedPathData(alternatingFlipPattern), /\bS\b/, "smooth rail output should not force reflected symmetric handles");
  assertNoDegenerateQuadraticCaps(alternatingFlipPattern, "curve pattern tiling/stacking");
  assertNodeTypesAligned(alternatingFlipPattern, "curve pattern tiling/stacking");
  assert.equal(centerlinePathCount(basePattern), 2, "curve pattern should emit one path per centerline row");
  assert.equal(
    centerlinePathCount(highQualityPattern),
    centerlinePathCount(basePattern),
    "output quality must not change centerline row path count",
  );
  assert.ok(
    smoothCurveCommandCount(firstGeneratedPathData(highQualityPattern)) >=
      smoothCurveCommandCount(firstGeneratedPathData(basePattern)),
    "output quality should preserve at least the base motif row detail after path simplification",
  );
  assert.equal(
    centerlinePathCount(moreTilesPattern),
    centerlinePathCount(basePattern),
    "tile count should preserve centerline row path count",
  );
  assert.equal(
    centerlinePathCount(connectedPattern),
    centerlinePathCount(basePattern),
    "connected motif tiles should emit one path per centerline row",
  );
  assert.doesNotMatch(
    connectedPattern,
    /fill-rule="evenodd"/,
    "connected motif tiles should not close individual source curves",
  );
  assert.ok(
    autoCoverageBounds.minX <= autoCoverageTolerance,
    `auto motif coverage should reach left edge (${autoCoverageBounds.minX})`,
  );
  assert.ok(
    autoCoverageBounds.maxX >= outputWidth - autoCoverageTolerance,
    `auto motif coverage should reach right edge (${autoCoverageBounds.maxX})`,
  );
  assert.ok(
    autoCoverageBounds.minY <= autoCoverageTolerance,
    `auto motif coverage should reach top edge (${autoCoverageBounds.minY})`,
  );
  assert.ok(
    autoCoverageBounds.maxY >= outputHeight - autoCoverageTolerance,
    `auto motif coverage should reach bottom edge (${autoCoverageBounds.maxY})`,
  );
  assert.ok(
    defaultFloodBounds.minX <= autoCoverageTolerance,
    `default motif flood should reach left edge (${defaultFloodBounds.minX})`,
  );
  assert.ok(
    defaultFloodBounds.maxX >= outputWidth - autoCoverageTolerance,
    `default motif flood should reach right edge (${defaultFloodBounds.maxX})`,
  );
  assert.ok(
    defaultFloodBounds.minY <= autoCoverageTolerance,
    `default motif flood should reach top edge (${defaultFloodBounds.minY})`,
  );
  assert.ok(
    defaultFloodBounds.maxY >= outputHeight - autoCoverageTolerance,
    `default motif flood should reach bottom edge (${defaultFloodBounds.maxY})`,
  );
  assert.equal(
    firstGeneratedPathData(defaultSpacingPattern),
    firstGeneratedPathData(explicitSpacingPattern),
    "legacy motif tile spacing should not change endpoint-chained rows",
  );
  assert.equal(
    commandCount(noGapPathData, "M"),
    1,
    "one motif row should render as one continuous outline, not separate gapped tile outlines",
  );
  for (const [label, bounds] of [
    ["flipped alternating motif tiles", alternatingFlipBounds],
    ["180-degree alternating motif tiles", alternatingRotateBounds],
  ]) {
    assert.ok(
      bounds.minX <= autoCoverageTolerance,
      `${label} should continue forward to the left edge instead of bouncing (${bounds.minX})`,
    );
    assert.ok(
      bounds.maxX >= outputWidth - autoCoverageTolerance,
      `${label} should continue forward to the right edge instead of bouncing (${bounds.maxX})`,
    );
  }
  assert.ok(
    clippedRotatedBounds.minX >= -autoCoverageTolerance * 2,
    `rotated motif output should be culled near the left artboard edge (${clippedRotatedBounds.minX})`,
  );
  assert.ok(
    clippedRotatedBounds.maxX <= outputWidth + autoCoverageTolerance * 2,
    `rotated motif output should be culled near the right artboard edge (${clippedRotatedBounds.maxX})`,
  );
  assert.ok(
    clippedRotatedBounds.minY >= -autoCoverageTolerance * 2,
    `rotated motif output should be culled near the top artboard edge (${clippedRotatedBounds.minY})`,
  );
  assert.ok(
    clippedRotatedBounds.maxY <= outputHeight + autoCoverageTolerance * 2,
    `rotated motif output should be culled near the bottom artboard edge (${clippedRotatedBounds.maxY})`,
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
    settings: { markMode: "curve", curveSpan: "cell-chain", sharedPath },
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
    "legacy connected cell chains should map to motif-pattern behavior",
  );
  assert.equal(
    firstGeneratedPathData(cellChain),
    firstGeneratedPathData(motifPattern),
    "legacy connected cell chains should preserve compatibility as motif-pattern output",
  );
};

const testBezziatorFixtureCurveRotations = () => {
  const pathData = bezziatorDocumentToPathData(loadBezziatorFixture());

  for (const layout of ["full-width", "full-height"]) {
    for (let rotation = 0; rotation <= 180; rotation += 15) {
      const label = `Bezziator fixture ${layout} rotation ${rotation}`;
      const svg = run({
        settings: {
          markMode: "curve",
          curveSpan: layout,
          sharedPath: pathData,
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
      label: "curve synchronized motif controls",
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
        curveSpan: "motif-pattern",
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
          motifCoverageMode: "manual",
          motifBleed: 2,
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
          motifCoverageMode: "manual",
          motifBleed: 2,
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
          motifCoverageMode: "manual",
          motifBleed: 2,
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
          motifCoverageMode: "manual",
          motifBleed: 2,
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

const testCrosshatchValueMapping = () => {
  assert.deepEqual(
    mapPixelToChannels([255, 255, 255, 255], "crosshatch-luminance"),
    { c: 0, m: 0, y: 0, k: 0 },
    "white should not activate crosshatch channels",
  );

  assert.deepEqual(
    mapPixelToChannels([0, 0, 0, 255], "crosshatch-luminance"),
    { c: 0.25, m: 0.25, y: 0.25, k: 0.25 },
    "black should split darkness across all crosshatch channels",
  );

  const lightGray = mapPixelToChannels([192, 192, 192, 255], "crosshatch-luminance");
  assert.ok(lightGray.k > 0 && lightGray.k < 0.25, "light gray should partially activate the first hatch layer");
  assert.equal(lightGray.c, 0, "light gray should not activate the second hatch layer");
  assert.equal(lightGray.m, 0, "light gray should not activate the third hatch layer");
  assert.equal(lightGray.y, 0, "light gray should not activate the fourth hatch layer");

  const darkGray = mapPixelToChannels([64, 64, 64, 255], "crosshatch-luminance");
  assert.equal(darkGray.k, 0.25, "dark gray should fill the first hatch layer budget");
  assert.equal(darkGray.c, 0.25, "dark gray should fill the second hatch layer budget");
  assert.ok(darkGray.m > 0.24 && darkGray.m < 0.25, "dark gray should nearly fill the third hatch layer budget");
  assert.equal(darkGray.y, 0, "dark gray should not activate the fourth hatch layer yet");

  assert.deepEqual(
    mapPixelToChannels([0, 0, 0, 255], "crosshatch-luminance", "k", ["c", "k"]),
    { c: 0.5, m: 0, y: 0, k: 0.5 },
    "black should split darkness across only enabled crosshatch channels",
  );

  assert.deepEqual(
    mapPixelToChannels([0, 0, 0, 255], "crosshatch-luminance", "k", ["m"]),
    { c: 0, m: 1, y: 0, k: 0 },
    "a single enabled crosshatch channel should receive full darkness",
  );

  const svg = run({
    settings: { valueMode: "crosshatch-luminance", markMode: "curve" },
    channels: enabledChannels,
  });
  assert.doesNotMatch(svg, /fill="#00aeef"|fill="#ec008c"|fill="#ffd400"/, "crosshatch output should be monochrome");
  assert.match(svg, /fill="#111111"/, "crosshatch output should use black hatch geometry");
};

const testExportBackgroundAndChannelFiltering = () => {
  const transparent = run({
    settings: { markMode: "shape", sharedPreset: "circle", sharedPath: "" },
    generatorOptions: { includeBackground: false },
  });
  assert.doesNotMatch(
    transparent,
    /<rect width="100%" height="100%" fill="white"\/>/,
    "transparent export should omit the white background rect",
  );

  const cyanOnly = run({
    settings: { markMode: "shape", sharedPreset: "circle", sharedPath: "" },
    channels: enabledChannels,
    generatorOptions: { includeBackground: false, renderChannelKeys: ["c"] },
  });
  assert.match(cyanOnly, /id="toniator-cyan"/, "channel-filtered export should include the requested channel");
  assert.doesNotMatch(cyanOnly, /id="toniator-magenta"|id="toniator-yellow"|id="toniator-black"/, "channel-filtered export should omit other channel groups");
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
testCrosshatchValueMapping();
testExportBackgroundAndChannelFiltering();
testShapeOffsetPeriodicity();
testAspectLocking();
testEditorCurveNormalization();

console.log("generator sweep tests passed");
