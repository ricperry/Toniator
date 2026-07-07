import { createSampleSource, loadSourceFile } from "./imageLoader.js";
import { aspectLockedDimensions } from "./aspect.js";
import { calculateGrid } from "./sampling.js";
import { getPresetNames, getPresetPath } from "./presets.js";
import { generateHalftoneSvg, renderMarkPreview } from "./svgGenerator.js";
import { loadCurvePathFromFile, svgSourceToPathData } from "./curveImport.js";
import { clearCurveEditor, mountCurveEditor } from "./curveEditor.js";
import { curveBounds, editableCurveToPathData, pathDataToEditableCurve } from "./curvePath.js";

const CHANNELS = {
  c: { name: "Cyan", color: "#00aeef", rotation: 15 },
  m: { name: "Magenta", color: "#ec008c", rotation: 75 },
  y: { name: "Yellow", color: "#ffd400", rotation: 0 },
  k: { name: "Black", color: "#111111", rotation: 45 },
};

const PRESET_FORMAT = "toniator-preset";
const PRESET_VERSION = 1;
const TESTING_PRESETS = [
  {
    name: "Rotated CMYK Circle Screens",
    description: "Shape-mode circles with independently rotated channel grids.",
    settings: {
      markMode: "shape",
      geometryMode: "shared",
      sharedPreset: "circle",
      valueMode: "cmyk",
      outputWidth: 900,
      outputHeight: 620,
      longEdgeCells: 92,
      gridScale: 76,
      minMark: 4,
      maxMark: 90,
      showBackground: true,
      preserveAspect: false,
    },
    channels: {
      c: { rotation: 0, gridRotation: 15, gridPivotX: -80, gridPivotY: 0, scale: 0.95, opacity: 0.72 },
      m: { rotation: 0, gridRotation: 75, gridPivotX: 70, gridPivotY: -20, scale: 0.95, opacity: 0.72 },
      y: { rotation: 0, gridRotation: 0, gridPivotX: 0, gridPivotY: 50, scale: 0.9, opacity: 0.68 },
      k: { rotation: 0, gridRotation: 45, gridPivotX: 20, gridPivotY: -70, scale: 1, opacity: 0.85 },
    },
  },
  {
    name: "Connected Curve Weave",
    description: "Motif curves chained across tiles with dense stacked rows.",
    settings: {
      markMode: "curve",
      curveSpan: "motif-pattern",
      syncCurveChannels: true,
      sharedPreset: "wave",
      sharedPath: "M -0.5 0 C -0.28 -0.32 0.28 0.32 0.5 0",
      sharedConnectEndpoints: true,
      sharedSmoothSeamTangents: true,
      valueMode: "cmyk",
      outputWidth: 900,
      outputHeight: 620,
      longEdgeCells: 88,
      gridScale: 70,
      minMark: 0,
      maxMark: 92,
      showBackground: true,
      preserveAspect: false,
    },
    channels: {
      c: { rotation: 0, gridRotation: 12, curveScale: 26, tileCount: 36, tileSpacing: 28, stackCount: 34, stackSpacing: 18, stackOffset: 0, alternateStackOffset: 12, connectEndpoints: true, smoothSeamTangents: true, opacity: 0.72 },
      m: { rotation: 32, gridRotation: -9, curveScale: 24, tileCount: 36, tileSpacing: 28, stackCount: 34, stackSpacing: 18, stackOffset: 7, alternateStackOffset: 8, connectEndpoints: true, smoothSeamTangents: true, opacity: 0.76 },
      y: { rotation: -18, gridRotation: 0, curveScale: 22, tileCount: 34, tileSpacing: 30, stackCount: 28, stackSpacing: 22, stackOffset: 3, alternateStackOffset: 0, connectEndpoints: true, smoothSeamTangents: true, opacity: 0.62 },
      k: { rotation: 47, gridRotation: 5, curveScale: 28, tileCount: 38, tileSpacing: 26, stackCount: 32, stackSpacing: 20, stackOffset: -4, alternateStackOffset: 10, connectEndpoints: true, smoothSeamTangents: true, opacity: 0.9 },
    },
  },
  {
    name: "Independent Curve Stress Test",
    description: "Independent source curves, mixed grid pivots, and varied motif controls.",
    settings: {
      markMode: "curve",
      curveSpan: "motif-pattern",
      syncCurveChannels: false,
      sharedPreset: "line",
      valueMode: "cmyk",
      outputWidth: 840,
      outputHeight: 560,
      longEdgeCells: 78,
      gridScale: 66,
      minMark: 3,
      maxMark: 96,
      showBackground: true,
      preserveAspect: false,
    },
    channels: {
      c: { customPath: "M -0.5 0 C -0.2 -0.28 0.2 0.28 0.5 0", rotation: 0, gridRotation: 18, gridPivotX: -110, gridPivotY: 50, curveScale: 22, tileCount: 24, tileSpacing: 31, stackCount: 22, stackSpacing: 21, alternateStackOffset: 12, connectEndpoints: true, smoothSeamTangents: true, opacity: 0.72 },
      m: { customPath: "M -0.5 0 L 0 0.24 L 0.5 0", rotation: 38, gridRotation: -14, gridPivotX: 80, gridPivotY: -60, curveScale: 24, tileCount: 24, tileSpacing: 32, stackCount: 22, stackSpacing: 22, alternateTileTransform: "rotate-180", connectEndpoints: false, smoothSeamTangents: false, opacity: 0.74 },
      y: { customPath: "M -0.5 0 C -0.12 0.36 0.12 -0.36 0.5 0", rotation: -24, gridRotation: 8, gridPivotX: 30, gridPivotY: 90, curveScale: 20, tileCount: 26, tileSpacing: 30, stackCount: 18, stackSpacing: 26, alternateTileTransform: "flip", connectEndpoints: false, smoothSeamTangents: false, opacity: 0.64 },
      k: { customPath: "M -0.5 -0.08 C -0.22 0.18 0.22 -0.18 0.5 0.08", rotation: 54, gridRotation: 0, gridPivotX: 0, gridPivotY: 0, curveScale: 28, tileCount: 28, tileSpacing: 28, stackCount: 24, stackSpacing: 20, alternateStackOffset: 10, connectEndpoints: true, smoothSeamTangents: true, opacity: 0.95 },
    },
  },
];

const state = {
  source: null,
  latestSvg: "",
  fitPreview: true,
  activeCurveEditTarget: null,
  infoOverlayMenuOpen: false,
  pivotDrag: null,
  infoOverlays: {
    gridPivots: true,
    gridRotation: true,
    channelGrid: false,
    tileVectors: true,
    stackVectors: true,
    channelOffsets: false,
  },
};

const els = {
  fileInput: document.querySelector("#fileInput"),
  sampleButton: document.querySelector("#sampleButton"),
  sourceInfo: document.querySelector("#sourceInfo"),
  testingPreset: document.querySelector("#testingPreset"),
  applyTestingPresetButton: document.querySelector("#applyTestingPresetButton"),
  savePresetButton: document.querySelector("#savePresetButton"),
  loadPresetButton: document.querySelector("#loadPresetButton"),
  presetFileInput: document.querySelector("#presetFileInput"),
  outputWidth: document.querySelector("#outputWidth"),
  outputHeight: document.querySelector("#outputHeight"),
  preserveAspect: document.querySelector("#preserveAspect"),
  longEdgeCells: document.querySelector("#longEdgeCells"),
  gridScale: document.querySelector("#gridScale"),
  minMark: document.querySelector("#minMark"),
  maxMark: document.querySelector("#maxMark"),
  valueMode: document.querySelector("#valueMode"),
  singleChannelRow: document.querySelector("#singleChannelRow"),
  singleChannel: document.querySelector("#singleChannel"),
  markMode: document.querySelector("#markMode"),
  curveSpanRow: document.querySelector("#curveSpanRow"),
  curveSpan: document.querySelector("#curveSpan"),
  curveTileCellsRow: document.querySelector("#curveTileCellsRow"),
  curveTileCells: document.querySelector("#curveTileCells"),
  syncCurveChannelsRow: document.querySelector("#syncCurveChannelsRow"),
  syncCurveChannels: document.querySelector("#syncCurveChannels"),
  curveModeHint: document.querySelector("#curveModeHint"),
  showBackground: document.querySelector("#showBackground"),
  geometryMode: document.querySelector("#geometryMode"),
  geometryModeHint: document.querySelector("#geometryModeHint"),
  sharedGeometryControls: document.querySelector("#sharedGeometryControls"),
  sharedPreset: document.querySelector("#sharedPreset"),
  sharedPath: document.querySelector("#sharedPath"),
  sharedCurveFile: document.querySelector("#sharedCurveFile"),
  sharedCurveEditControls: document.querySelector("#sharedCurveEditControls"),
  sharedCurveEditButton: document.querySelector("#sharedCurveEditButton"),
  sharedConnectEndpoints: document.querySelector("#sharedConnectEndpoints"),
  sharedSmoothSeam: document.querySelector("#sharedSmoothSeam"),
  sharedCurvePreview: document.querySelector("#sharedCurvePreview"),
  channelSettingsTarget: document.querySelector("#channelSettingsTarget"),
  cmykDeltaControls: document.querySelector("#cmykDeltaControls"),
  channelControls: document.querySelector("#channelControls"),
  resetDefaultsButton: document.querySelector("#resetDefaultsButton"),
  exportButton: document.querySelector("#exportButton"),
  exportPngButton: document.querySelector("#exportPngButton"),
  pngExportDialog: document.querySelector("#pngExportDialog"),
  pngExportInfo: document.querySelector("#pngExportInfo"),
  pngExportDpi: document.querySelector("#pngExportDpi"),
  pngExportWidth: document.querySelector("#pngExportWidth"),
  pngExportHeight: document.querySelector("#pngExportHeight"),
  pngExportLockAspect: document.querySelector("#pngExportLockAspect"),
  pngExportCancelButton: document.querySelector("#pngExportCancelButton"),
  pngExportConfirmButton: document.querySelector("#pngExportConfirmButton"),
  fitPreviewButton: document.querySelector("#fitPreviewButton"),
  previewFrame: document.querySelector("#previewFrame"),
  infoOverlayMenu: document.querySelector("#infoOverlayMenu"),
  overlayGridPivots: document.querySelector("#overlayGridPivots"),
  overlayGridRotation: document.querySelector("#overlayGridRotation"),
  overlayChannelGrid: document.querySelector("#overlayChannelGrid"),
  overlayTileVectors: document.querySelector("#overlayTileVectors"),
  overlayStackVectors: document.querySelector("#overlayStackVectors"),
  overlayChannelOffsets: document.querySelector("#overlayChannelOffsets"),
  renderStats: document.querySelector("#renderStats"),
  previewMount: document.querySelector("#previewMount"),
  emptyState: document.querySelector("#emptyState"),
  curveEditOverlay: document.querySelector("#curveEditOverlay"),
  curveEditOverlayTitle: document.querySelector("#curveEditOverlayTitle"),
  curveEditOverlayMount: document.querySelector("#curveEditOverlayMount"),
  curveEditOverlayClose: document.querySelector("#curveEditOverlayClose"),
};

init();

function init() {
  renderTestingPresetOptions();
  renderSharedPresetOptions();
  renderChannelControls();
  enhanceBoundedNumberControls();
  applyControlHints();
  bindEvents();
  applyUrlParams();
  updateModeUi();
  updateMarkPreviews();
  updateFitPreviewButton();
  setExportButtonsEnabled(false);
  loadSample();
}

function bindEvents() {
  els.fileInput.addEventListener("change", handleFileChange);
  els.sharedCurveFile.addEventListener("change", handleSharedCurveFileChange);
  els.channelControls.addEventListener("change", handleChannelCurveFileChange);
  els.channelControls.addEventListener("click", handleChannelControlsClick);
  els.sharedCurveEditButton.addEventListener("click", () => {
    state.activeCurveEditTarget =
      state.activeCurveEditTarget === "shared" ? null : "shared";
    updateModeUi();
    updateMarkPreviews();
  });
  els.sampleButton.addEventListener("click", loadSample);
  els.applyTestingPresetButton.addEventListener("click", applySelectedTestingPreset);
  els.savePresetButton.addEventListener("click", savePresetFile);
  els.loadPresetButton.addEventListener("click", () => els.presetFileInput.click());
  els.presetFileInput.addEventListener("change", handlePresetFileChange);
  els.curveEditOverlayClose.addEventListener("click", () => {
    state.activeCurveEditTarget = null;
    updateModeUi();
    updateMarkPreviews();
  });
  els.resetDefaultsButton.addEventListener("click", resetDefaults);
  els.exportButton.addEventListener("click", exportSvg);
  els.exportPngButton.addEventListener("click", openPngExportDialog);
  els.pngExportCancelButton.addEventListener("click", () => els.pngExportDialog.close());
  els.pngExportConfirmButton.addEventListener("click", exportPngFromDialog);
  els.pngExportDpi.addEventListener("input", updatePngSizeFromDpi);
  els.pngExportWidth.addEventListener("input", () => updatePngSizeFromDimension("width"));
  els.pngExportHeight.addEventListener("input", () => updatePngSizeFromDimension("height"));
  bindOverlayMenu();
  els.fitPreviewButton.addEventListener("click", () => {
    state.fitPreview = !state.fitPreview;
    els.previewMount.classList.toggle("actual-size", !state.fitPreview);
    updateFitPreviewButton();
    updatePreviewScale();
  });
  window.addEventListener("resize", () => {
    updatePreviewScale();
    updateInfoOverlayLayout();
    updateCurveEditOverlayLayout();
  });
  window.addEventListener("keydown", handleGlobalKeyDown);

  for (const input of document.querySelectorAll("input, select, textarea")) {
    if (input === els.fileInput || input === els.presetFileInput || input.dataset.boundRange === "true") continue;
    input.addEventListener("input", () => {
      if (input === els.outputWidth || input === els.outputHeight) {
        applyAspectFromEditedDimension(input === els.outputHeight ? "height" : "width");
      }
      if (input === els.markMode) {
        renderSharedPresetOptions();
        renderChannelPresetOptions();
      }
      if (
        els.markMode.value === "curve" &&
        els.syncCurveChannels.checked &&
        (input === els.syncCurveChannels || input === els.sharedPreset || input === els.sharedPath)
      ) {
        syncAllChannelCurves(resolveBaseCurvePath());
      }
      if (input === els.syncCurveChannels) {
        state.activeCurveEditTarget = els.syncCurveChannels.checked ? "shared" : null;
      }
      updateModeUi();
      updateMarkPreviews();
      renderInfoOverlay();
      queueRender();
    });
  }
}

function bindOverlayMenu() {
  const bindings = [
    [els.overlayGridPivots, "gridPivots"],
    [els.overlayGridRotation, "gridRotation"],
    [els.overlayChannelGrid, "channelGrid"],
    [els.overlayTileVectors, "tileVectors"],
    [els.overlayStackVectors, "stackVectors"],
    [els.overlayChannelOffsets, "channelOffsets"],
  ];

  for (const [input, key] of bindings) {
    input.checked = state.infoOverlays[key];
    input.addEventListener("input", () => {
      state.infoOverlays[key] = input.checked;
      renderInfoOverlay();
    });
  }
}

function handleGlobalKeyDown(event) {
  if (event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement || event.target instanceof HTMLSelectElement) {
    return;
  }
  if (event.key.toLowerCase() !== "n") return;
  state.infoOverlayMenuOpen = !state.infoOverlayMenuOpen;
  els.infoOverlayMenu.classList.toggle("hidden", !state.infoOverlayMenuOpen);
  event.preventDefault();
}

function enhanceBoundedNumberControls() {
  for (const input of document.querySelectorAll('input[type="number"][min][max]')) {
    if (input.dataset.sliderEnhanced === "true") continue;
    const range = document.createElement("input");
    range.type = "range";
    range.min = input.min;
    range.max = input.max;
    range.step = input.step || "1";
    range.value = input.value || input.min;
    range.dataset.boundRange = "true";
    range.className = "control-range";

    const wrap = document.createElement("div");
    wrap.className = "slider-control";
    input.parentNode.insertBefore(wrap, input);
    wrap.append(range, input);
    input.dataset.sliderEnhanced = "true";
    input.classList.add("slider-number");

    range.addEventListener("input", () => {
      input.value = range.value;
      input.dispatchEvent(new Event("input", { bubbles: true }));
    });
    input.addEventListener("input", () => syncBoundRangeForNumber(input));
  }
}

function syncBoundRangeForNumber(input) {
  if (!input?.dataset || input.dataset.sliderEnhanced !== "true") return;
  const range = input.parentElement?.querySelector('[data-bound-range="true"]');
  if (!range) return;
  range.value = input.value;
}

function applyControlHints() {
  for (const control of document.querySelectorAll("input, select, textarea")) {
    const hint = controlHint(control.id);
    if (!hint) continue;
    control.title = hint;
    const label = control.closest("label");
    if (label) label.title = hint;
  }

  for (const range of document.querySelectorAll('[data-bound-range="true"]')) {
    const number = range.parentElement?.querySelector(".slider-number");
    if (number?.title) range.title = number.title;
  }
}

function controlHint(id) {
  const exact = {
    outputWidth: "Output artboard width in SVG document units.",
    outputHeight: "Output artboard height in SVG document units.",
    longEdgeCells: "Sets the base sampling grid density on the source image's long edge.",
    gridScale: "Controls the filled portion of each halftone cell before per-channel sizing.",
    minMark: "Smallest mark size generated from sampled values.",
    maxMark: "Largest mark size generated from sampled values.",
    curveTileCells: "Length, in grid cells, used by tiled full-width/full-height curve layouts.",
    testingPreset: "Loads a saved test scenario that exercises a known combination of modes and channel settings.",
    valueMode: "Chooses how source pixels are converted into channel values before thresholding and mark sizing. Crosshatching mode splits grayscale density across the enabled channels as monochrome hatch layers.",
    singleChannel: "Chooses which CMYK channel receives grayscale values in single-channel mode.",
    markMode: "Switches between repeated shape marks and variable-width curve marks.",
    curveSpan: "Chooses whether curves span the document, tile across an axis, or use motif tiling/stacking.",
    syncCurveChannels: "Uses one shared source curve for all channels while preserving each channel's independent render controls.",
    showBackground: "Shows the loaded source image/SVG behind the generated preview for visual alignment.",
    geometryMode: "Chooses whether shape-mode mark geometry is shared by all channels or set independently per channel.",
    sharedPreset: "Preset geometry used for synchronized shape marks or synchronized source curves.",
    sharedPath: "Optional SVG path data used instead of the synchronized preset.",
    sharedConnectEndpoints: "Connects curve tiles to neighboring tiles during rendering.",
    sharedSmoothSeam: "Aligns seam tangents when connected curve tiles render.",
    "cmyk-rotationDelta": "Adds this many degrees to every channel's shape or source-curve rotation at render time.",
    "cmyk-gridRotationDelta": "Adds this many degrees to every channel's sampling grid rotation without changing saved channel values.",
    "cmyk-gridPivotXDelta": "Adds a horizontal artboard-space offset to every channel's grid rotation pivot.",
    "cmyk-gridPivotYDelta": "Adds a vertical artboard-space offset to every channel's grid rotation pivot.",
    "cmyk-scaleMultiplier": "Multiplies every channel's generated mark width scale.",
    "cmyk-curveScaleMultiplier": "Multiplies every channel's source-curve scale for curve rendering.",
    "cmyk-resolutionMultiplier": "Multiplies every channel's sampling density.",
    "cmyk-outputQualityMultiplier": "Multiplies every channel's variable-width curve resampling quality.",
    "cmyk-maxSizeMultiplier": "Multiplies every channel's maximum mark size cap.",
    "cmyk-opacityMultiplier": "Multiplies every channel's preview/render opacity.",
    "cmyk-offsetXDelta": "Adds a horizontal offset to every channel's phase or curve placement.",
    "cmyk-offsetYDelta": "Adds a vertical offset to every channel's phase or curve placement.",
    "cmyk-tileCountDelta": "Adds to every channel's curve motif tile count.",
    "cmyk-tileSpacingDelta": "Adds artboard-space distance between motif tile origins for every channel.",
    "cmyk-tileAngleDelta": "Adds degrees to every channel's motif tile direction.",
    "cmyk-tileOffsetDelta": "Adds offset along each channel's tile direction.",
    "cmyk-stackCountDelta": "Adds to every channel's motif stack row count.",
    "cmyk-stackSpacingDelta": "Adds artboard-space distance between stack rows for every channel.",
    "cmyk-stackAngleDelta": "Adds degrees to every channel's motif stack direction.",
    "cmyk-stackOffsetDelta": "Adds offset along each channel's stack direction.",
    "cmyk-alternateStackOffsetDelta": "Adds alternating-row tile-direction offset for every channel's motif stacks.",
  };
  if (exact[id]) return exact[id];

  const suffixHints = [
    ["-enabled", "Toggles whether this channel contributes to the rendered halftone."],
    ["-color", "Rendered color for this channel and its preview overlays."],
    ["-rotation", "Rotates the mark or source curve shape within each channel."],
    ["-gridRotation", "Rotates this channel's sampling and placement grid around its grid pivot."],
    ["-gridPivotX", "Horizontal offset of the grid rotation pivot from the artboard center. Drag the matching preview pivot to edit it visually."],
    ["-gridPivotY", "Vertical offset of the grid rotation pivot from the artboard center. Drag the matching preview pivot to edit it visually."],
    ["-scale", "Scales generated mark width for this channel."],
    ["-threshold", "Suppresses marks below this sampled value."],
    ["-resolutionScale", "Multiplies this channel's image sampling density."],
    ["-outputQuality", "Controls resampling density for variable-width curve outlines."],
    ["-maxSize", "Caps this channel's largest generated mark size."],
    ["-opacity", "Preview/render opacity for this channel."],
    ["-offsetX", "Horizontal phase/placement offset for this channel."],
    ["-offsetY", "Vertical phase/placement offset for this channel."],
    ["-curveScale", "Scales source curves for full-document and motif curve rendering."],
    ["-tileCount", "Number of curve motif tiles emitted per stack row."],
    ["-tileSpacing", "Distance between curve motif tile origins."],
    ["-tileAngle", "Direction used to advance curve motif tiles."],
    ["-tileOffset", "Offset along the tile direction."],
    ["-stackCount", "Number of stacked rows for curve motif patterns."],
    ["-stackSpacing", "Distance between stack rows."],
    ["-stackAngle", "Direction used to advance stack rows."],
    ["-stackOffset", "Offset along the stack direction."],
    ["-alternateStackOffset", "Additional tile-direction offset applied to alternating stack rows."],
    ["-connectEndpoints", "Connects this channel's curve tile endpoints to neighboring tiles in the rendered output."],
    ["-smoothSeam", "Smooths endpoint tangents where connected curve tiles meet."],
  ];

  const match = suffixHints.find(([suffix]) => id.endsWith(suffix));
  if (match) return match[1];
  if (id.startsWith("cmyk-")) return "Combined CMYK delta applied at render time without overwriting the individual channel setting.";
  return "";
}

function renderTestingPresetOptions() {
  els.testingPreset.innerHTML = TESTING_PRESETS
    .map((preset, index) => `<option value="${index}">${preset.name}</option>`)
    .join("");
}

function applySelectedTestingPreset() {
  const preset = TESTING_PRESETS[Number(els.testingPreset.value)] ?? TESTING_PRESETS[0];
  applyPresetDocument(preset);
  els.renderStats.textContent = `Applied testing preset: ${preset.name}`;
}

function savePresetFile() {
  const preset = {
    format: PRESET_FORMAT,
    version: PRESET_VERSION,
    name: sourceBaseName() ? `${sourceBaseName()} Toniator preset` : "Toniator preset",
    savedAt: new Date().toISOString(),
    settings: {
      ...readSettings(),
      preserveAspect: els.preserveAspect.checked,
    },
    cmykDeltas: readCmykDeltas(),
    channels: readBaseChannels(),
  };
  const blob = new Blob([`${JSON.stringify(preset, null, 2)}\n`], {
    type: "application/json;charset=utf-8",
  });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = `${sanitizeFileName(preset.name)}.tntr`;
  link.click();
  URL.revokeObjectURL(url);
}

async function handlePresetFileChange(event) {
  const [file] = event.target.files;
  if (!file) return;

  try {
    const preset = parsePresetDocument(await file.text());
    applyPresetDocument(preset);
    els.renderStats.textContent = `Loaded preset: ${preset.name || file.name}`;
  } catch (error) {
    console.error(error);
    els.renderStats.textContent = `Could not load preset: ${error.message}`;
  } finally {
    event.target.value = "";
  }
}

function parsePresetDocument(source) {
  const preset = JSON.parse(source);
  if (!preset || typeof preset !== "object") {
    throw new Error("Preset file must contain a JSON object.");
  }
  if (preset.format && preset.format !== PRESET_FORMAT) {
    throw new Error(`Unsupported preset format: ${preset.format}`);
  }
  if (!preset.settings || !preset.channels) {
    throw new Error("Preset must include settings and channels.");
  }
  return preset;
}

function applyPresetDocument(preset) {
  resetDefaults({ renderAfter: false });
  applySettingsPreset(preset.settings ?? {});
  applyCmykDeltaPreset(preset.cmykDeltas ?? {});
  renderSharedPresetOptions();
  renderChannelPresetOptions();
  applyChannelsPreset(preset.channels ?? {});

  if (els.markMode.value === "curve" && els.syncCurveChannels.checked) {
    syncAllChannelCurves(resolveBaseCurvePath());
  }

  state.activeCurveEditTarget = null;
  finalizePresetControlChanges();
}

function applyCmykDeltaPreset(deltas) {
  setControlValue(document.querySelector("#cmyk-rotationDelta"), deltas.rotationDelta);
  setControlValue(document.querySelector("#cmyk-gridRotationDelta"), deltas.gridRotationDelta);
  setControlValue(document.querySelector("#cmyk-gridPivotXDelta"), deltas.gridPivotXDelta);
  setControlValue(document.querySelector("#cmyk-gridPivotYDelta"), deltas.gridPivotYDelta);
  setControlValue(document.querySelector("#cmyk-scaleMultiplier"), deltas.scaleMultiplier);
  setControlValue(document.querySelector("#cmyk-curveScaleMultiplier"), deltas.curveScaleMultiplier);
  setControlValue(document.querySelector("#cmyk-resolutionMultiplier"), deltas.resolutionMultiplier);
  setControlValue(document.querySelector("#cmyk-outputQualityMultiplier"), deltas.outputQualityMultiplier);
  setControlValue(document.querySelector("#cmyk-maxSizeMultiplier"), deltas.maxSizeMultiplier);
  setControlValue(document.querySelector("#cmyk-opacityMultiplier"), deltas.opacityMultiplier);
  setControlValue(document.querySelector("#cmyk-offsetXDelta"), deltas.offsetXDelta);
  setControlValue(document.querySelector("#cmyk-offsetYDelta"), deltas.offsetYDelta);
  setControlValue(document.querySelector("#cmyk-tileCountDelta"), deltas.tileCountDelta);
  setControlValue(document.querySelector("#cmyk-tileSpacingDelta"), deltas.tileSpacingDelta);
  setControlValue(document.querySelector("#cmyk-tileAngleDelta"), deltas.tileAngleDelta);
  setControlValue(document.querySelector("#cmyk-tileOffsetDelta"), deltas.tileOffsetDelta);
  setControlValue(document.querySelector("#cmyk-stackCountDelta"), deltas.stackCountDelta);
  setControlValue(document.querySelector("#cmyk-stackSpacingDelta"), deltas.stackSpacingDelta);
  setControlValue(document.querySelector("#cmyk-stackAngleDelta"), deltas.stackAngleDelta);
  setControlValue(document.querySelector("#cmyk-stackOffsetDelta"), deltas.stackOffsetDelta);
  setControlValue(document.querySelector("#cmyk-alternateStackOffsetDelta"), deltas.alternateStackOffsetDelta);
}

function applySettingsPreset(settings) {
  setControlValue(els.outputWidth, settings.outputWidth);
  setControlValue(els.outputHeight, settings.outputHeight);
  setControlValue(els.longEdgeCells, settings.longEdgeCells);
  setControlValue(els.gridScale, settings.gridScale);
  setControlValue(els.minMark, settings.minMark);
  setControlValue(els.maxMark, settings.maxMark);
  setControlValue(els.valueMode, settings.valueMode);
  setControlValue(els.singleChannel, settings.singleChannel);
  setControlValue(els.markMode, settings.markMode);
  setControlValue(els.curveSpan, settings.curveSpan);
  setControlValue(els.curveTileCells, settings.curveTileCells);
  setControlValue(els.geometryMode, settings.geometryMode);
  setControlValue(els.sharedPreset, settings.sharedPreset);
  setControlValue(els.sharedPath, settings.sharedPath);
  setControlValue(els.preserveAspect, settings.preserveAspect);
  setControlValue(els.syncCurveChannels, settings.syncCurveChannels);
  setControlValue(els.sharedConnectEndpoints, settings.sharedConnectEndpoints);
  setControlValue(els.sharedSmoothSeam, settings.sharedSmoothSeamTangents);
  setControlValue(els.showBackground, settings.showBackground);
}

function applyChannelsPreset(channels) {
  for (const key of Object.keys(CHANNELS)) {
    const channel = channels[key];
    if (!channel) continue;

    setControlValue(document.querySelector(`#${key}-enabled`), channel.enabled);
    setControlValue(document.querySelector(`#${key}-color`), channel.color);
    setControlValue(document.querySelector(`#${key}-rotation`), channel.rotation);
    setControlValue(document.querySelector(`#${key}-gridRotation`), channel.gridRotation);
    setControlValue(document.querySelector(`#${key}-gridPivotX`), channel.gridPivotX);
    setControlValue(document.querySelector(`#${key}-gridPivotY`), channel.gridPivotY);
    setControlValue(document.querySelector(`#${key}-scale`), channel.scale);
    setControlValue(document.querySelector(`#${key}-curveScale`), channel.curveScale);
    setControlValue(document.querySelector(`#${key}-tileCount`), channel.tileCount);
    setControlValue(document.querySelector(`#${key}-tileSpacing`), channel.tileSpacing);
    setControlValue(document.querySelector(`#${key}-tileAngle`), channel.tileAngle);
    setControlValue(document.querySelector(`#${key}-tileOffset`), channel.tileOffset);
    setControlValue(document.querySelector(`#${key}-stackCount`), channel.stackCount);
    setControlValue(document.querySelector(`#${key}-stackSpacing`), channel.stackSpacing);
    setControlValue(document.querySelector(`#${key}-stackAngle`), channel.stackAngle);
    setControlValue(document.querySelector(`#${key}-stackOffset`), channel.stackOffset);
    setControlValue(document.querySelector(`#${key}-alternateStackOffset`), channel.alternateStackOffset);
    setControlValue(document.querySelector(`#${key}-alternateTileTransform`), channel.alternateTileTransform);
    setControlValue(document.querySelector(`#${key}-outputQuality`), channel.outputQuality);
    setControlValue(document.querySelector(`#${key}-threshold`), percentageValue(channel.threshold));
    setControlValue(document.querySelector(`#${key}-maxSize`), channel.maxSize);
    setControlValue(document.querySelector(`#${key}-resolutionScale`), channel.resolutionScale);
    setControlValue(document.querySelector(`#${key}-offsetX`), channel.offsetX);
    setControlValue(document.querySelector(`#${key}-offsetY`), channel.offsetY);
    setControlValue(document.querySelector(`#${key}-opacity`), percentageValue(channel.opacity));
    setControlValue(document.querySelector(`#${key}-preset`), channel.preset);
    setControlValue(document.querySelector(`#${key}-path`), channel.customPath);
    setControlValue(document.querySelector(`#${key}-connectEndpoints`), channel.connectEndpoints);
    setControlValue(document.querySelector(`#${key}-smoothSeam`), channel.smoothSeamTangents);
  }
}

function finalizePresetControlChanges() {
  if (state.source && els.preserveAspect.checked) {
    applyAspectFromEditedDimension("width");
  }
  updateModeUi();
  updateMarkPreviews();

  if (state.source) {
    render();
  }
}

function setControlValue(control, value) {
  if (!control || value === undefined || value === null) return;

  if (control.type === "checkbox" || control.type === "radio") {
    control.checked = Boolean(value);
    return;
  }

  if (control.tagName === "SELECT") {
    const values = Array.from(control.options).map((option) => option.value);
    if (values.includes(String(value))) {
      control.value = String(value);
    }
    return;
  }

  control.value = String(value);
  syncBoundRangeForNumber(control);
}

function percentageValue(value) {
  const number = Number(value);
  if (!Number.isFinite(number)) return value;
  return number <= 1 ? number * 100 : number;
}

function resetDefaults({ renderAfter = true } = {}) {
  window.clearTimeout(renderTimer);
  state.activeCurveEditTarget = null;
  state.fitPreview = true;
  els.previewMount.classList.remove("actual-size");

  for (const control of document.querySelectorAll("input, select, textarea")) {
    resetControl(control);
  }

  renderSharedPresetOptions();
  renderChannelPresetOptions();
  resetSelectToDefault(els.sharedPreset);
  for (const key of Object.keys(CHANNELS)) {
    resetSelectToDefault(document.querySelector(`#${key}-preset`));
  }

  if (els.markMode.value === "curve" && els.syncCurveChannels.checked) {
    syncAllChannelCurves(resolveBaseCurvePath());
  }

  if (state.source && els.preserveAspect.checked) {
    applyAspectFromEditedDimension("width");
  }

  updateModeUi();
  updateMarkPreviews();
  updateFitPreviewButton();

  if (state.source && renderAfter) {
    render();
  } else if (!state.source) {
    setExportButtonsEnabled(false);
    els.renderStats.textContent = "Load a source file to begin.";
  }
}

function resetControl(control) {
  if (control.type === "file") {
    control.value = "";
    return;
  }

  if (control.type === "checkbox" || control.type === "radio") {
    control.checked = control.defaultChecked;
    return;
  }

  if (control.tagName === "SELECT") {
    resetSelectToDefault(control);
    return;
  }

  control.value = control.defaultValue;
}

function resetSelectToDefault(select) {
  if (!select) return;
  const defaultIndex = Array.from(select.options).findIndex((option) => option.defaultSelected);
  select.selectedIndex = defaultIndex >= 0 ? defaultIndex : 0;
}

async function handleSharedCurveFileChange(event) {
  const [file] = event.target.files;
  if (!file) return;

  try {
    els.sharedPath.value = await loadCurvePathFromFile(file);
    els.markMode.value = "curve";
    state.activeCurveEditTarget = els.syncCurveChannels.checked ? "shared" : state.activeCurveEditTarget;
    syncAllChannelCurves(els.sharedPath.value);
    renderSharedPresetOptions();
    renderChannelPresetOptions();
    updateModeUi();
    updateMarkPreviews();
    queueRender();
  } catch (error) {
    console.error(error);
    els.renderStats.textContent = `Could not import curve: ${error.message}`;
  }
}

async function handleChannelCurveFileChange(event) {
  const input = event.target.closest?.("input[data-curve-file]");
  if (!input) return;

  const [file] = input.files;
  if (!file) return;

  try {
    const key = input.dataset.curveFile;
    const path = await loadCurvePathFromFile(file);
    document.querySelector(`#${key}-path`).value = path;
    if (els.syncCurveChannels.checked) {
      syncAllChannelCurves(path);
      els.sharedPath.value = path;
    }
    els.markMode.value = "curve";
    renderSharedPresetOptions();
    renderChannelPresetOptions();
    updateModeUi();
    updateMarkPreviews();
    queueRender();
  } catch (error) {
    console.error(error);
    els.renderStats.textContent = `Could not import channel curve: ${error.message}`;
  }
}

function applyUrlParams() {
  const params = new URLSearchParams(window.location.search);
  setSelectFromParam(els.markMode, params.get("markMode"));

  if (params.has("markMode")) {
    renderSharedPresetOptions();
    renderChannelPresetOptions();
  }

  setSelectFromParam(els.curveSpan, params.get("curveSpan"));
  setSelectFromParam(els.geometryMode, params.get("geometryMode"));
  setSelectFromParam(els.valueMode, params.get("valueMode"));
  setSelectFromParam(els.singleChannel, params.get("singleChannel"));
  setSelectFromParam(els.sharedPreset, params.get("sharedPreset"));

  setInputValueFromParam(els.sharedPath, params.get("sharedPath"));
  setInputValueFromParam(els.outputWidth, params.get("outputWidth"));
  setInputValueFromParam(els.outputHeight, params.get("outputHeight"));
  setInputValueFromParam(els.longEdgeCells, params.get("cells"));
  setInputValueFromParam(els.gridScale, params.get("gridScale"));
  setInputValueFromParam(els.minMark, params.get("minMark"));
  setInputValueFromParam(els.maxMark, params.get("maxMark"));
  setInputValueFromParam(els.curveTileCells, params.get("curveTileCells"));
  applyChannelUrlParams(params);

  if (params.has("preserveAspect")) {
    els.preserveAspect.checked = params.get("preserveAspect") !== "false";
  }

  if (params.has("background")) {
    els.showBackground.checked = params.get("background") !== "false";
  }

  if (params.has("syncCurveChannels")) {
    els.syncCurveChannels.checked = params.get("syncCurveChannels") !== "false";
  }

  if (params.has("sharedConnect")) {
    els.sharedConnectEndpoints.checked = params.get("sharedConnect") !== "false";
  }

  if (params.has("sharedSmoothSeam")) {
    els.sharedSmoothSeam.checked = params.get("sharedSmoothSeam") !== "false";
  }

  if (params.has("editCurve")) {
    const target = params.get("editCurve");
    state.activeCurveEditTarget = ["shared", ...Object.keys(CHANNELS)].includes(target)
      ? target
      : null;
  }
}

function handleChannelControlsClick(event) {
  const button = event.target.closest?.("button[data-edit-curve]");
  if (!button) return;

  const key = button.dataset.editCurve;
  if (els.syncCurveChannels.checked) {
    state.activeCurveEditTarget =
      state.activeCurveEditTarget === "shared" ? null : "shared";
  } else {
    state.activeCurveEditTarget =
      state.activeCurveEditTarget === key ? null : key;
  }

  updateModeUi();
  updateMarkPreviews();
}

function applyChannelUrlParams(params) {
  for (const key of Object.keys(CHANNELS)) {
    if (params.has(`${key}Enabled`)) {
      document.querySelector(`#${key}-enabled`).checked =
        params.get(`${key}Enabled`) !== "false";
    }
    setInputValueFromParam(
      document.querySelector(`#${key}-color`),
      params.get(`${key}Color`),
    );
    setInputValueFromParam(
      document.querySelector(`#${key}-offsetX`),
      params.get(`${key}OffsetX`),
    );
    setInputValueFromParam(
      document.querySelector(`#${key}-offsetY`),
      params.get(`${key}OffsetY`),
    );
    setInputValueFromParam(
      document.querySelector(`#${key}-rotation`),
      params.get(`${key}Rotation`),
    );
    setInputValueFromParam(
      document.querySelector(`#${key}-gridRotation`),
      params.get(`${key}GridRotation`),
    );
    setInputValueFromParam(
      document.querySelector(`#${key}-gridPivotX`),
      params.get(`${key}GridPivotX`),
    );
    setInputValueFromParam(
      document.querySelector(`#${key}-gridPivotY`),
      params.get(`${key}GridPivotY`),
    );
    setInputValueFromParam(
      document.querySelector(`#${key}-scale`),
      params.get(`${key}Scale`),
    );
    setInputValueFromParam(
      document.querySelector(`#${key}-curveScale`),
      params.get(`${key}CurveScale`),
    );
    setInputValueFromParam(
      document.querySelector(`#${key}-tileCount`),
      params.get(`${key}TileCount`),
    );
    setInputValueFromParam(
      document.querySelector(`#${key}-tileSpacing`),
      params.get(`${key}TileSpacing`),
    );
    setInputValueFromParam(
      document.querySelector(`#${key}-tileAngle`),
      params.get(`${key}TileAngle`),
    );
    setInputValueFromParam(
      document.querySelector(`#${key}-tileOffset`),
      params.get(`${key}TileOffset`),
    );
    setInputValueFromParam(
      document.querySelector(`#${key}-stackCount`),
      params.get(`${key}StackCount`),
    );
    setInputValueFromParam(
      document.querySelector(`#${key}-stackSpacing`),
      params.get(`${key}StackSpacing`),
    );
    setInputValueFromParam(
      document.querySelector(`#${key}-stackAngle`),
      params.get(`${key}StackAngle`),
    );
    setInputValueFromParam(
      document.querySelector(`#${key}-stackOffset`),
      params.get(`${key}StackOffset`),
    );
    setInputValueFromParam(
      document.querySelector(`#${key}-alternateStackOffset`),
      params.get(`${key}AlternateStackOffset`),
    );
    setInputValueFromParam(
      document.querySelector(`#${key}-outputQuality`),
      params.get(`${key}OutputQuality`),
    );
    setSelectFromParam(
      document.querySelector(`#${key}-alternateTileTransform`),
      params.get(`${key}AlternateTileTransform`),
    );
    setInputValueFromParam(
      document.querySelector(`#${key}-threshold`),
      params.get(`${key}Threshold`),
    );
    setInputValueFromParam(
      document.querySelector(`#${key}-maxSize`),
      params.get(`${key}MaxSize`),
    );
    setInputValueFromParam(
      document.querySelector(`#${key}-resolutionScale`),
      params.get(`${key}Resolution`),
    );
    setInputValueFromParam(
      document.querySelector(`#${key}-opacity`),
      params.get(`${key}Opacity`),
    );
    setSelectFromParam(
      document.querySelector(`#${key}-preset`),
      params.get(`${key}Preset`),
    );
    setInputValueFromParam(
      document.querySelector(`#${key}-path`),
      params.get(`${key}Path`),
    );
    if (params.has(`${key}Connect`)) {
      document.querySelector(`#${key}-connectEndpoints`).checked =
        params.get(`${key}Connect`) !== "false";
    }
    if (params.has(`${key}SmoothSeam`)) {
      document.querySelector(`#${key}-smoothSeam`).checked =
        params.get(`${key}SmoothSeam`) !== "false";
    }
  }
}

function setSelectFromParam(select, value) {
  if (!value) return;
  const values = Array.from(select.options).map((option) => option.value);
  if (values.includes(value)) {
    select.value = value;
  }
}

function setInputValueFromParam(input, value) {
  if (value !== null && value !== "") {
    input.value = value;
  }
}

async function loadSample() {
  try {
    els.sourceInfo.textContent = "Loading built-in sample…";
    state.source = await createSampleSource();
    els.sourceInfo.textContent = `${state.source.fileName} · ${state.source.width}×${state.source.height} · sample SVG`;
    if (els.preserveAspect.checked) {
      applyAspectFromEditedDimension("width");
    }
    await render();
  } catch (error) {
    console.error(error);
    els.sourceInfo.textContent = `Could not load sample: ${error.message}`;
  }
}

async function handleFileChange(event) {
  const [file] = event.target.files;
  if (!file) return;

  try {
    els.sourceInfo.textContent = "Loading source…";
    state.source = await loadSourceFile(file);
    els.sourceInfo.textContent = `${state.source.fileName} · ${Math.round(state.source.width)}×${Math.round(state.source.height)} · ${state.source.type.toUpperCase()}`;
    if (els.preserveAspect.checked) {
      applyAspectFromEditedDimension("width");
    }
    await render();
  } catch (error) {
    console.error(error);
    els.sourceInfo.textContent = `Could not load source: ${error.message}`;
  }
}

let renderTimer = 0;
function queueRender() {
  window.clearTimeout(renderTimer);
  renderTimer = window.setTimeout(() => {
    if (state.source) render();
  }, 180);
}

async function render() {
  if (!state.source) {
    els.renderStats.textContent = "Load a source file to begin.";
    return;
  }

  const settings = readSettings();
  const grid = calculateGrid(
    state.source,
    settings.outputWidth,
    settings.outputHeight,
    settings.longEdgeCells,
  );
  const channels = readChannels();

  const started = performance.now();
  state.latestSvg = generateHalftoneSvg({
    source: state.source,
    grid,
    settings,
    channels,
    includePreviewBackground: settings.showBackground,
  });
  const elapsed = performance.now() - started;

  els.previewMount.innerHTML = state.latestSvg;
  updatePreviewScale();
  renderInfoOverlay();
  updateCurveEditOverlayLayout();
  els.emptyState.classList.add("hidden");
  setExportButtonsEnabled(true);
  els.renderStats.textContent = `${grid.cols}×${grid.rows} base cells · ${describeChannelResolutions(channels, settings.longEdgeCells)} · rendered in ${elapsed.toFixed(1)}ms`;
}

function exportSvg() {
  if (!state.latestSvg) return;

  const svg = exportSvgMarkup();
  const blob = new Blob([svg], { type: "image/svg+xml;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = `${sourceBaseName()}-toniator-halftone.svg`;
  link.click();
  URL.revokeObjectURL(url);
}

function openPngExportDialog() {
  if (!state.latestSvg) return;

  const defaults = defaultPngExportSize();
  setControlValue(els.pngExportDpi, defaults.dpi);
  setControlValue(els.pngExportWidth, defaults.width);
  setControlValue(els.pngExportHeight, defaults.height);
  els.pngExportInfo.textContent = pngExportInfoText(defaults);
  if (typeof els.pngExportDialog.showModal === "function") {
    els.pngExportDialog.showModal();
  } else {
    els.pngExportDialog.setAttribute("open", "");
  }
}

async function exportPngFromDialog() {
  if (!state.latestSvg) return;

  const width = clampInteger(readNumber(els.pngExportWidth, readSettings().outputWidth), 1, 32000);
  const height = clampInteger(readNumber(els.pngExportHeight, readSettings().outputHeight), 1, 32000);
  const dpi = clampInteger(readNumber(els.pngExportDpi, 300), 1, 2400);
  els.pngExportDialog.close();

  const svg = exportSvgMarkup();
  const svgBlob = new Blob([svg], { type: "image/svg+xml;charset=utf-8" });
  const svgUrl = URL.createObjectURL(svgBlob);

  try {
    const image = await loadImage(svgUrl);
    const canvas = document.createElement("canvas");
    canvas.width = width;
    canvas.height = height;
    const context = canvas.getContext("2d");
    context.drawImage(image, 0, 0, canvas.width, canvas.height);
    const pngBlob = await addPngDpiMetadata(await canvasToBlob(canvas, "image/png"), dpi);
    const pngUrl = URL.createObjectURL(pngBlob);
    const link = document.createElement("a");
    link.href = pngUrl;
    link.download = `${sourceBaseName()}-toniator-halftone.png`;
    link.click();
    URL.revokeObjectURL(pngUrl);
  } finally {
    URL.revokeObjectURL(svgUrl);
  }
}

function defaultPngExportSize() {
  const settings = readSettings();
  const physicalSize = sourcePhysicalSize();
  const dpi = Math.max(1, Math.round(physicalSize.dpi || 300));
  return {
    dpi,
    width: Math.max(1, Math.round(physicalSize.widthInches * dpi)),
    height: Math.max(1, Math.round(physicalSize.heightInches * dpi)),
    physicalSize,
    outputAspect: settings.outputWidth / settings.outputHeight,
  };
}

function updatePngSizeFromDpi() {
  const physicalSize = sourcePhysicalSize();
  const dpi = clampInteger(readNumber(els.pngExportDpi, 300), 1, 2400);
  setControlValue(els.pngExportWidth, Math.max(1, Math.round(physicalSize.widthInches * dpi)));
  setControlValue(els.pngExportHeight, Math.max(1, Math.round(physicalSize.heightInches * dpi)));
  els.pngExportInfo.textContent = pngExportInfoText({ dpi, physicalSize });
}

function updatePngSizeFromDimension(dimension) {
  const physicalSize = sourcePhysicalSize();
  const width = clampInteger(readNumber(els.pngExportWidth, 1), 1, 32000);
  const height = clampInteger(readNumber(els.pngExportHeight, 1), 1, 32000);
  const aspect = physicalSize.widthInches / physicalSize.heightInches;

  if (els.pngExportLockAspect.checked && dimension === "width") {
    const nextHeight = Math.max(1, Math.round(width / aspect));
    setControlValue(els.pngExportHeight, nextHeight);
    setControlValue(els.pngExportDpi, Math.max(1, Math.round(width / physicalSize.widthInches)));
    return;
  }

  if (els.pngExportLockAspect.checked && dimension === "height") {
    const nextWidth = Math.max(1, Math.round(height * aspect));
    setControlValue(els.pngExportWidth, nextWidth);
    setControlValue(els.pngExportDpi, Math.max(1, Math.round(height / physicalSize.heightInches)));
    return;
  }

  const dpiFromEditedAxis =
    dimension === "width"
      ? width / physicalSize.widthInches
      : height / physicalSize.heightInches;
  setControlValue(els.pngExportDpi, Math.max(1, Math.round(dpiFromEditedAxis)));
}

function sourcePhysicalSize() {
  const source = state.source;
  if (source?.physicalSize?.widthInches > 0 && source?.physicalSize?.heightInches > 0) {
    return {
      ...source.physicalSize,
      dpi:
        source.physicalSize.dpi ||
        averageDpi(source.physicalSize.dpiX, source.physicalSize.dpiY) ||
        96,
    };
  }

  const settings = readSettings();
  return {
    widthInches: settings.outputWidth / 96,
    heightInches: settings.outputHeight / 96,
    dpi: 96,
    source: "pixel",
  };
}

function pngExportInfoText({ dpi, physicalSize }) {
  const basis =
    physicalSize.source === "embedded"
      ? `embedded size ${formatNumber(physicalSize.widthInches)}×${formatNumber(physicalSize.heightInches)} in`
      : `pixel size treated as ${formatNumber(physicalSize.widthInches)}×${formatNumber(physicalSize.heightInches)} in at 96 DPI`;
  return `${basis} · ${Math.round(dpi)} DPI`;
}

function averageDpi(dpiX, dpiY) {
  const values = [dpiX, dpiY].filter((value) => Number.isFinite(value) && value > 0);
  if (values.length === 0) return 0;
  return values.reduce((sum, value) => sum + value, 0) / values.length;
}

function exportSvgMarkup(exportSettings = readSettings()) {
  const grid = calculateGrid(
    state.source,
    exportSettings.outputWidth,
    exportSettings.outputHeight,
    exportSettings.longEdgeCells,
  );
  return generateHalftoneSvg({
    source: state.source,
    grid,
    settings: exportSettings,
    channels: readChannels(),
    includePreviewBackground: false,
  });
}

function loadImage(url) {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error("Could not rasterize SVG for PNG export."));
    image.src = url;
  });
}

function canvasToBlob(canvas, type) {
  return new Promise((resolve, reject) => {
    canvas.toBlob((blob) => {
      if (blob) {
        resolve(blob);
      } else {
        reject(new Error("Could not create PNG export."));
      }
    }, type);
  });
}

async function addPngDpiMetadata(blob, dpi) {
  const source = new Uint8Array(await blob.arrayBuffer());
  if (!isPngBytes(source)) return blob;

  const pixelsPerMeter = Math.max(1, Math.round(dpi / 0.0254));
  const chunk = createPngPhysChunk(pixelsPerMeter, pixelsPerMeter);
  const insertOffset = 8 + 25;
  const result = new Uint8Array(source.length + chunk.length);
  result.set(source.slice(0, insertOffset), 0);
  result.set(chunk, insertOffset);
  result.set(source.slice(insertOffset), insertOffset + chunk.length);
  return new Blob([result], { type: "image/png" });
}

function isPngBytes(bytes) {
  const signature = [137, 80, 78, 71, 13, 10, 26, 10];
  return bytes.length > 33 && signature.every((byte, index) => bytes[index] === byte);
}

function createPngPhysChunk(xPixelsPerMeter, yPixelsPerMeter) {
  const type = new Uint8Array([112, 72, 89, 115]);
  const data = new Uint8Array(9);
  writeUint32(data, 0, xPixelsPerMeter);
  writeUint32(data, 4, yPixelsPerMeter);
  data[8] = 1;

  const chunk = new Uint8Array(21);
  writeUint32(chunk, 0, data.length);
  chunk.set(type, 4);
  chunk.set(data, 8);
  writeUint32(chunk, 17, crc32(concatBytes(type, data)));
  return chunk;
}

function concatBytes(a, b) {
  const result = new Uint8Array(a.length + b.length);
  result.set(a, 0);
  result.set(b, a.length);
  return result;
}

function writeUint32(bytes, offset, value) {
  bytes[offset] = (value >>> 24) & 0xff;
  bytes[offset + 1] = (value >>> 16) & 0xff;
  bytes[offset + 2] = (value >>> 8) & 0xff;
  bytes[offset + 3] = value & 0xff;
}

function crc32(bytes) {
  let crc = 0xffffffff;
  for (const byte of bytes) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit += 1) {
      crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0);
    }
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function setExportButtonsEnabled(enabled) {
  els.exportButton.disabled = !enabled;
  els.exportPngButton.disabled = !enabled;
}

function renderInfoOverlay() {
  const artSvg = els.previewMount.querySelector("svg:not(.info-overlay-svg)");
  els.previewMount.querySelector(".info-overlay-svg")?.remove();
  if (!artSvg || !state.source) return;

  const settings = readSettings();
  const channels = readChannels();
  const baseGrid = calculateGrid(
    state.source,
    settings.outputWidth,
    settings.outputHeight,
    settings.longEdgeCells,
  );
  const overlay = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  overlay.setAttribute("class", "info-overlay-svg");
  overlay.setAttribute("viewBox", `0 0 ${settings.outputWidth} ${settings.outputHeight}`);
  overlay.setAttribute("aria-hidden", "true");

  for (const [key, channel] of Object.entries(channels)) {
    if (!channel.enabled) continue;
    appendChannelInfoOverlay(overlay, key, channel, settings, baseGrid);
  }

  els.previewMount.append(overlay);
  updateInfoOverlayLayout();
}

function updateInfoOverlayLayout() {
  const artSvg = els.previewMount.querySelector("svg:not(.info-overlay-svg)");
  const overlay = els.previewMount.querySelector(".info-overlay-svg");
  if (!artSvg || !overlay) return;

  const artRect = artSvg.getBoundingClientRect();
  const mountRect = els.previewMount.getBoundingClientRect();
  overlay.style.left = `${artRect.left - mountRect.left}px`;
  overlay.style.top = `${artRect.top - mountRect.top}px`;
  overlay.style.width = `${artRect.width}px`;
  overlay.style.height = `${artRect.height}px`;
}

function updateCurveEditOverlayLayout() {
  if (els.curveEditOverlay.classList.contains("hidden")) return;
  const artSvg = els.previewMount.querySelector("svg:not(.info-overlay-svg)");
  const mount = els.curveEditOverlayMount;
  if (!artSvg || !mount) return;

  const artRect = artSvg.getBoundingClientRect();
  const frameRect = els.previewFrame.getBoundingClientRect();
  mount.style.left = `${artRect.left - frameRect.left + els.previewFrame.scrollLeft}px`;
  mount.style.top = `${artRect.top - frameRect.top + els.previewFrame.scrollTop}px`;
  mount.style.width = `${artRect.width}px`;
  mount.style.height = `${artRect.height}px`;
}

function appendChannelInfoOverlay(svg, key, channel, settings, baseGrid) {
  const color = channel.color || CHANNELS[key].color;
  const center = { x: settings.outputWidth / 2, y: settings.outputHeight / 2 };
  const pivot = {
    x: center.x + finite(channel.gridPivotX),
    y: center.y + finite(channel.gridPivotY),
  };
  const grid = channelGridForOverlay(baseGrid, channel);
  const vectorLength = Math.max(28, Math.min(settings.outputWidth, settings.outputHeight) * 0.09);
  const gridDirection = unitVector(finite(channel.gridRotation));
  const tileDirection = unitVector(finite(channel.tileAngle));
  const stackDirection = unitVector(finite(channel.stackAngle, 90));
  const markCenter = {
    x: center.x + finite(channel.offsetX),
    y: center.y + finite(channel.offsetY),
  };

  if (state.infoOverlays.channelGrid) {
    appendGridLines(svg, settings, grid, channel, pivot, color);
  }

  if (state.infoOverlays.gridRotation) {
    appendVector(svg, pivot, gridDirection, vectorLength, color, "grid", `${CHANNELS[key].name} grid rotation`);
  }

  if (state.infoOverlays.tileVectors && settings.markMode === "curve") {
    appendVector(svg, markCenter, tileDirection, vectorLength * 0.82, color, "tile", `${CHANNELS[key].name} tile direction`);
  }

  if (state.infoOverlays.stackVectors && settings.markMode === "curve") {
    appendVector(svg, markCenter, stackDirection, vectorLength * 0.68, color, "stack", `${CHANNELS[key].name} stack direction`);
  }

  if (state.infoOverlays.channelOffsets) {
    appendOffsetIndicator(svg, center, markCenter, color, `${CHANNELS[key].name} channel offset`);
  }

  if (state.infoOverlays.gridPivots) {
    appendPivot(svg, key, pivot, color);
  }
}

function appendGridLines(svg, settings, grid, channel, pivot, color) {
  const group = svgElement("g", { class: "info-grid-lines", stroke: color });
  const spacing = Math.max(2, Math.min(grid.cellWidth, grid.cellHeight));
  const skip = Math.max(1, Math.ceil(Math.max(settings.outputWidth, settings.outputHeight) / spacing / 36));
  const radius = Math.hypot(settings.outputWidth, settings.outputHeight);
  const bounds = {
    minX: -radius,
    maxX: settings.outputWidth + radius,
    minY: -radius,
    maxY: settings.outputHeight + radius,
  };

  for (let x = signedGridStart(bounds.minX, spacing) + finite(channel.offsetX); x <= bounds.maxX; x += spacing * skip) {
    const p1 = rotatePoint({ x, y: bounds.minY }, pivot, finite(channel.gridRotation));
    const p2 = rotatePoint({ x, y: bounds.maxY }, pivot, finite(channel.gridRotation));
    group.append(svgElement("line", { x1: p1.x, y1: p1.y, x2: p2.x, y2: p2.y }));
  }

  for (let y = signedGridStart(bounds.minY, spacing) + finite(channel.offsetY); y <= bounds.maxY; y += spacing * skip) {
    const p1 = rotatePoint({ x: bounds.minX, y }, pivot, finite(channel.gridRotation));
    const p2 = rotatePoint({ x: bounds.maxX, y }, pivot, finite(channel.gridRotation));
    group.append(svgElement("line", { x1: p1.x, y1: p1.y, x2: p2.x, y2: p2.y }));
  }

  svg.append(group);
}

function appendVector(svg, origin, direction, length, color, kind, label) {
  const end = {
    x: origin.x + direction.x * length,
    y: origin.y + direction.y * length,
  };
  const group = svgElement("g", { class: `info-vector info-vector-${kind}`, stroke: color, fill: color });
  const titleNode = svgElement("title", {});
  titleNode.textContent = label;
  group.append(titleNode);
  group.append(svgElement("line", { x1: origin.x, y1: origin.y, x2: end.x, y2: end.y }));
  const headA = rotateVector({ x: -10, y: -4 }, Math.atan2(direction.y, direction.x) * 180 / Math.PI);
  const headB = rotateVector({ x: -10, y: 4 }, Math.atan2(direction.y, direction.x) * 180 / Math.PI);
  group.append(svgElement("path", {
    d: `M ${formatNumber(end.x)} ${formatNumber(end.y)} L ${formatNumber(end.x + headA.x)} ${formatNumber(end.y + headA.y)} L ${formatNumber(end.x + headB.x)} ${formatNumber(end.y + headB.y)} Z`,
  }));
  svg.append(group);
}

function appendPivot(svg, key, pivot, color) {
  const group = svgElement("g", { class: "info-pivot", fill: color, stroke: color });
  const titleNode = svgElement("title", {});
  titleNode.textContent = `${CHANNELS[key].name} grid pivot. Drag to move.`;
  group.append(titleNode);
  group.append(svgElement("line", { x1: pivot.x - 11, y1: pivot.y, x2: pivot.x + 11, y2: pivot.y }));
  group.append(svgElement("line", { x1: pivot.x, y1: pivot.y - 11, x2: pivot.x, y2: pivot.y + 11 }));
  const handle = svgElement("circle", {
    class: "info-pivot-handle",
    cx: pivot.x,
    cy: pivot.y,
    r: 8,
    "data-channel": key,
  });
  handle.addEventListener("pointerdown", handlePivotPointerDown);
  group.append(handle);
  svg.append(group);
}

function appendOffsetIndicator(svg, center, markCenter, color, label) {
  const group = svgElement("g", { class: "info-offset-vector", stroke: color, fill: color });
  const titleNode = svgElement("title", {});
  titleNode.textContent = label;
  group.append(titleNode);
  group.append(svgElement("line", { x1: center.x, y1: center.y, x2: markCenter.x, y2: markCenter.y }));
  group.append(svgElement("circle", { cx: markCenter.x, cy: markCenter.y, r: 4 }));
  svg.append(group);
}

function handlePivotPointerDown(event) {
  const channelKey = event.currentTarget.dataset.channel;
  const overlay = event.currentTarget.ownerSVGElement;
  if (!channelKey || !overlay) return;
  event.preventDefault();
  event.currentTarget.setPointerCapture?.(event.pointerId);
  const geometry = overlayDragGeometry(overlay);
  const channels = readBaseChannels();
  state.pivotDrag = {
    channelKey,
    geometry,
    startPoint: overlayPointFromGeometry(geometry, event),
    startOffsetX: finite(channels[channelKey]?.offsetX),
    startOffsetY: finite(channels[channelKey]?.offsetY),
  };

  const move = (moveEvent) => {
    if (!state.pivotDrag) return;
    const point = overlayPointFromGeometry(state.pivotDrag.geometry, moveEvent);
    setChannelPivotFromDocumentPoint(channelKey, point, state.pivotDrag);
    renderInfoOverlay();
    queueRender();
  };
  const up = () => {
    state.pivotDrag = null;
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", up);
  };

  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", up);
}

function setChannelPivotFromDocumentPoint(channelKey, point, dragState = null) {
  const settings = readSettings();
  const deltas = readCmykDeltas();
  const nextX = point.x - settings.outputWidth / 2 - deltas.gridPivotXDelta;
  const nextY = point.y - settings.outputHeight / 2 - deltas.gridPivotYDelta;
  setControlValue(document.querySelector(`#${channelKey}-gridPivotX`), roundForInput(nextX));
  setControlValue(document.querySelector(`#${channelKey}-gridPivotY`), roundForInput(nextY));

  if (dragState?.startPoint) {
    const deltaX = point.x - dragState.startPoint.x;
    const deltaY = point.y - dragState.startPoint.y;
    setControlValue(
      document.querySelector(`#${channelKey}-offsetX`),
      roundForInput(dragState.startOffsetX + deltaX),
    );
    setControlValue(
      document.querySelector(`#${channelKey}-offsetY`),
      roundForInput(dragState.startOffsetY + deltaY),
    );
  }
}

function channelGridForOverlay(baseGrid, channel) {
  const multiplier = Number.isFinite(channel.resolutionScale) ? channel.resolutionScale : 1;
  return {
    cellWidth: baseGrid.cellWidth / Math.max(0.05, multiplier),
    cellHeight: baseGrid.cellHeight / Math.max(0.05, multiplier),
  };
}

function overlayDragGeometry(svg) {
  const rect = svg.getBoundingClientRect();
  const viewBox = svg.viewBox?.baseVal;
  return {
    left: rect.left,
    top: rect.top,
    width: Math.max(1, rect.width),
    height: Math.max(1, rect.height),
    minX: viewBox?.x || 0,
    minY: viewBox?.y || 0,
    viewWidth: viewBox?.width || rect.width || 1,
    viewHeight: viewBox?.height || rect.height || 1,
  };
}

function overlayPointFromGeometry(geometry, event) {
  return {
    x: geometry.minX + ((event.clientX - geometry.left) / geometry.width) * geometry.viewWidth,
    y: geometry.minY + ((event.clientY - geometry.top) / geometry.height) * geometry.viewHeight,
  };
}

function signedGridStart(value, spacing) {
  return Math.floor(value / spacing) * spacing;
}

function svgElement(name, attrs = {}) {
  const element = document.createElementNS("http://www.w3.org/2000/svg", name);
  for (const [key, value] of Object.entries(attrs)) {
    element.setAttribute(key, String(value));
  }
  return element;
}

function unitVector(degrees) {
  const radians = (degrees * Math.PI) / 180;
  return { x: Math.cos(radians), y: Math.sin(radians) };
}

function roundForInput(value) {
  return Number.parseFloat(Number(value).toFixed(2));
}

function formatNumber(value) {
  return Number.parseFloat(Number(value).toFixed(3));
}

function readSettings() {
  return {
    outputWidth: readNumber(els.outputWidth, 900),
    outputHeight: readNumber(els.outputHeight, 600),
    longEdgeCells: readNumber(els.longEdgeCells, 90),
    gridScale: readNumber(els.gridScale, 92),
    minMark: readNumber(els.minMark, 0),
    maxMark: readNumber(els.maxMark, 85),
    valueMode: els.valueMode.value,
    singleChannel: els.singleChannel.value,
    markMode: els.markMode.value,
    curveSpan: els.curveSpan.value,
    curveTileCells: readNumber(els.curveTileCells, 12),
    syncCurveChannels: els.syncCurveChannels.checked,
    sharedConnectEndpoints: els.sharedConnectEndpoints.checked,
    sharedSmoothSeamTangents:
      els.sharedConnectEndpoints.checked && els.sharedSmoothSeam.checked,
    showBackground: els.showBackground.checked,
    geometryMode: els.geometryMode.value,
    useSharedMark: els.markMode.value !== "curve" && els.geometryMode.value === "shared",
    sharedPreset: els.sharedPreset.value,
    sharedPath: els.sharedPath.value,
  };
}

function readChannels() {
  return applyCmykDeltas(readBaseChannels(), readCmykDeltas());
}

function readBaseChannels() {
  const channels = {};
  for (const key of Object.keys(CHANNELS)) {
    channels[key] = {
      enabled: document.querySelector(`#${key}-enabled`).checked,
      color: document.querySelector(`#${key}-color`).value,
      rotation: readNumber(document.querySelector(`#${key}-rotation`), 0),
      gridRotation: readNumber(document.querySelector(`#${key}-gridRotation`), 0),
      gridPivotX: readNumber(document.querySelector(`#${key}-gridPivotX`), 0),
      gridPivotY: readNumber(document.querySelector(`#${key}-gridPivotY`), 0),
      scale: readNumber(document.querySelector(`#${key}-scale`), 1),
      curveScale: readNumber(document.querySelector(`#${key}-curveScale`), 32),
      tileCount: readNumber(document.querySelector(`#${key}-tileCount`), 1),
      tileSpacing: readNumber(document.querySelector(`#${key}-tileSpacing`), 36),
      tileAngle: readNumber(document.querySelector(`#${key}-tileAngle`), 0),
      tileOffset: readNumber(document.querySelector(`#${key}-tileOffset`), 0),
      stackCount: readNumber(document.querySelector(`#${key}-stackCount`), 1),
      stackSpacing: readNumber(document.querySelector(`#${key}-stackSpacing`), 36),
      stackAngle: readNumber(document.querySelector(`#${key}-stackAngle`), 90),
      stackOffset: readNumber(document.querySelector(`#${key}-stackOffset`), 0),
      alternateStackOffset: readNumber(document.querySelector(`#${key}-alternateStackOffset`), 0),
      alternateTileTransform: document.querySelector(`#${key}-alternateTileTransform`).value,
      outputQuality: readNumber(document.querySelector(`#${key}-outputQuality`), 1),
      threshold: readNumber(document.querySelector(`#${key}-threshold`), 0) / 100,
      maxSize: readNumber(document.querySelector(`#${key}-maxSize`), 100),
      resolutionScale: readNumber(document.querySelector(`#${key}-resolutionScale`), 1),
      offsetX: readNumber(document.querySelector(`#${key}-offsetX`), 0),
      offsetY: readNumber(document.querySelector(`#${key}-offsetY`), 0),
      opacity: readNumber(document.querySelector(`#${key}-opacity`), 100) / 100,
      preset: document.querySelector(`#${key}-preset`).value,
      customPath: document.querySelector(`#${key}-path`).value,
      connectEndpoints: document.querySelector(`#${key}-connectEndpoints`).checked,
      smoothSeamTangents:
        document.querySelector(`#${key}-connectEndpoints`).checked &&
        document.querySelector(`#${key}-smoothSeam`).checked,
    };
  }
  return channels;
}

function readCmykDeltas() {
  return {
    rotationDelta: readNumber(document.querySelector("#cmyk-rotationDelta"), 0),
    gridRotationDelta: readNumber(document.querySelector("#cmyk-gridRotationDelta"), 0),
    gridPivotXDelta: readNumber(document.querySelector("#cmyk-gridPivotXDelta"), 0),
    gridPivotYDelta: readNumber(document.querySelector("#cmyk-gridPivotYDelta"), 0),
    scaleMultiplier: readNumber(document.querySelector("#cmyk-scaleMultiplier"), 1),
    curveScaleMultiplier: readNumber(document.querySelector("#cmyk-curveScaleMultiplier"), 1),
    resolutionMultiplier: readNumber(document.querySelector("#cmyk-resolutionMultiplier"), 1),
    outputQualityMultiplier: readNumber(document.querySelector("#cmyk-outputQualityMultiplier"), 1),
    maxSizeMultiplier: readNumber(document.querySelector("#cmyk-maxSizeMultiplier"), 1),
    opacityMultiplier: readNumber(document.querySelector("#cmyk-opacityMultiplier"), 1),
    offsetXDelta: readNumber(document.querySelector("#cmyk-offsetXDelta"), 0),
    offsetYDelta: readNumber(document.querySelector("#cmyk-offsetYDelta"), 0),
    tileCountDelta: readNumber(document.querySelector("#cmyk-tileCountDelta"), 0),
    tileSpacingDelta: readNumber(document.querySelector("#cmyk-tileSpacingDelta"), 0),
    tileAngleDelta: readNumber(document.querySelector("#cmyk-tileAngleDelta"), 0),
    tileOffsetDelta: readNumber(document.querySelector("#cmyk-tileOffsetDelta"), 0),
    stackCountDelta: readNumber(document.querySelector("#cmyk-stackCountDelta"), 0),
    stackSpacingDelta: readNumber(document.querySelector("#cmyk-stackSpacingDelta"), 0),
    stackAngleDelta: readNumber(document.querySelector("#cmyk-stackAngleDelta"), 0),
    stackOffsetDelta: readNumber(document.querySelector("#cmyk-stackOffsetDelta"), 0),
    alternateStackOffsetDelta: readNumber(document.querySelector("#cmyk-alternateStackOffsetDelta"), 0),
  };
}

function applyCmykDeltas(channels, deltas) {
  return Object.fromEntries(
    Object.entries(channels).map(([key, channel]) => [
      key,
      {
        ...channel,
        rotation: channel.rotation + deltas.rotationDelta,
        gridRotation: channel.gridRotation + deltas.gridRotationDelta,
        gridPivotX: channel.gridPivotX + deltas.gridPivotXDelta,
        gridPivotY: channel.gridPivotY + deltas.gridPivotYDelta,
        scale: channel.scale * deltas.scaleMultiplier,
        curveScale: channel.curveScale * deltas.curveScaleMultiplier,
        resolutionScale: channel.resolutionScale * deltas.resolutionMultiplier,
        outputQuality: channel.outputQuality * deltas.outputQualityMultiplier,
        maxSize: channel.maxSize * deltas.maxSizeMultiplier,
        opacity: clamp(channel.opacity * deltas.opacityMultiplier, 0, 1),
        offsetX: channel.offsetX + deltas.offsetXDelta,
        offsetY: channel.offsetY + deltas.offsetYDelta,
        tileCount: Math.max(1, channel.tileCount + deltas.tileCountDelta),
        tileSpacing: Math.max(0, channel.tileSpacing + deltas.tileSpacingDelta),
        tileAngle: channel.tileAngle + deltas.tileAngleDelta,
        tileOffset: channel.tileOffset + deltas.tileOffsetDelta,
        stackCount: Math.max(1, channel.stackCount + deltas.stackCountDelta),
        stackSpacing: Math.max(0, channel.stackSpacing + deltas.stackSpacingDelta),
        stackAngle: channel.stackAngle + deltas.stackAngleDelta,
        stackOffset: channel.stackOffset + deltas.stackOffsetDelta,
        alternateStackOffset: channel.alternateStackOffset + deltas.alternateStackOffsetDelta,
      },
    ]),
  );
}

function renderSharedPresetOptions() {
  const mode = els.markMode.value;
  const names = getPresetNames(mode);
  const current = els.sharedPreset.value || names[0];
  els.sharedPreset.innerHTML = names
    .map((name) => `<option value="${name}">${title(name)}</option>`)
    .join("");
  els.sharedPreset.value = names.includes(current) ? current : names[0];
}

function renderChannelControls() {
  els.channelControls.innerHTML = Object.entries(CHANNELS)
    .map(
      ([key, defaults]) => `
      <article class="channel-card" data-channel-card="${key}">
        <div class="channel-title">
          <h3>${defaults.name}</h3>
          <label class="check">
            <input id="${key}-enabled" type="checkbox" checked />
            Enabled
          </label>
        </div>
        <button type="button" class="secondary small curve-edit-button" data-edit-curve="${key}">
          Edit ${defaults.name} Curve
        </button>
        <div id="${key}-preview" class="mark-preview"></div>
        <div class="grid two">
          <label data-channel-color>Color <input id="${key}-color" type="color" value="${defaults.color}" /></label>
          <label>Shape/Curve Rotation ° <input id="${key}-rotation" type="number" min="-360" max="360" step="1" value="${defaults.rotation}" /></label>
        </div>
        <div class="curve-pattern-controls channel-grid-controls">
          <h4>Channel Grid Controls</h4>
          <label>Grid Rotation ° <input id="${key}-gridRotation" type="number" min="-360" max="360" step="1" value="0" /></label>
          <div class="grid two" data-grid-pivot-controls="${key}">
            <label>Grid Pivot X <input id="${key}-gridPivotX" type="number" min="-4000" max="4000" step="0.5" value="0" /></label>
            <label>Grid Pivot Y <input id="${key}-gridPivotY" type="number" min="-4000" max="4000" step="0.5" value="0" /></label>
          </div>
        </div>
        <div class="grid two">
          <label>Width Scale <input id="${key}-scale" type="number" min="0" max="5" step="0.05" value="1" /></label>
          <label>Threshold % <input id="${key}-threshold" type="number" min="0" max="100" step="1" value="4" /></label>
        </div>
        <div class="grid two">
          <label>Halftone Sample Density × <input id="${key}-resolutionScale" type="number" min="0.1" max="4" step="0.1" value="1" /></label>
          <label data-mode="curve">Curve Output Quality × <input id="${key}-outputQuality" type="number" min="0.25" max="6" step="0.25" value="1" /></label>
        </div>
        <div class="grid two">
          <label>Max size % <input id="${key}-maxSize" type="number" min="0" max="240" step="1" value="100" /></label>
          <label>Opacity % <input id="${key}-opacity" type="number" min="0" max="100" step="1" value="92" /></label>
        </div>
        <div class="grid two">
          <label>X offset <input id="${key}-offsetX" type="number" min="-200" max="200" step="0.5" value="0" /></label>
          <label>Y offset <input id="${key}-offsetY" type="number" min="-200" max="200" step="0.5" value="0" /></label>
        </div>
        <p class="hint" data-mode="shape">In shape mode, offsets are wrapped grid phases; mark placement and image sampling move together.</p>
        <div class="curve-pattern-controls" data-mode="curve" data-curve-layout="motif-pattern">
          <h4>Source Curve Scale</h4>
          <div class="grid two">
            <label>Source Curve Scale <input id="${key}-curveScale" type="number" min="0.1" max="500" step="0.5" value="32" /></label>
            <span></span>
          </div>
          <h4>Tile Controls</h4>
          <div class="grid two">
            <label>Tile Count <input id="${key}-tileCount" type="number" min="1" max="200" step="1" value="1" /></label>
            <label>Tile Spacing <input id="${key}-tileSpacing" type="number" min="0" max="1000" step="0.5" value="36" /></label>
          </div>
          <div class="grid two">
            <label>Tile Angle ° <input id="${key}-tileAngle" type="number" min="-360" max="360" step="1" value="0" /></label>
            <label>Tile Offset <input id="${key}-tileOffset" type="number" min="-1000" max="1000" step="0.5" value="0" /></label>
          </div>
          <label>Alternate Tile Transform
            <select id="${key}-alternateTileTransform">
              <option value="none">None</option>
              <option value="flip">Flip alternating tiles</option>
              <option value="rotate-180">Rotate alternating tiles 180°</option>
            </select>
          </label>
          <h4>Stack Controls</h4>
          <div class="grid two">
            <label>Stack Count <input id="${key}-stackCount" type="number" min="1" max="200" step="1" value="1" /></label>
            <label>Stack Spacing <input id="${key}-stackSpacing" type="number" min="0" max="1000" step="0.5" value="36" /></label>
          </div>
          <div class="grid two">
            <label>Stack Angle ° <input id="${key}-stackAngle" type="number" min="-360" max="360" step="1" value="90" /></label>
            <label>Stack Offset <input id="${key}-stackOffset" type="number" min="-1000" max="1000" step="0.5" value="0" /></label>
          </div>
          <label>Alternate Stack Offset <input id="${key}-alternateStackOffset" type="number" min="-1000" max="1000" step="0.5" value="0" /></label>
        </div>
        <div class="channel-mark">
          <p class="hint channel-mark-hint">Channel ${defaults.name} mark geometry. Custom path overrides the preset.</p>
          <label data-mode="shape">Preset <select id="${key}-preset"></select></label>
          <label data-mode="shape">Custom path <span class="muted">(optional SVG d)</span><textarea id="${key}-path" rows="2" spellcheck="false"></textarea></label>
          <label data-mode="curve">Import Bezziator/SVG curve <input id="${key}-curveFile" data-curve-file="${key}" type="file" accept=".bezvg,.bezziator,.svg,image/svg+xml" /></label>
        </div>
        <div class="curve-continuity">
          <label class="check">
            <input id="${key}-connectEndpoints" type="checkbox" />
            Connect endpoints
          </label>
          <label class="check">
            <input id="${key}-smoothSeam" type="checkbox" checked />
            Smooth seam tangents
          </label>
        </div>
      </article>
    `,
    )
    .join("");
  renderChannelPresetOptions();
  syncAllChannelCurves(resolveBaseCurvePath());
}

function renderChannelPresetOptions() {
  const mode = els.markMode.value;
  const names = getPresetNames(mode);
  for (const key of Object.keys(CHANNELS)) {
    const select = document.querySelector(`#${key}-preset`);
    const current = select.value || names[0];
    select.innerHTML = names
      .map((name) => `<option value="${name}">${title(name)}</option>`)
      .join("");
    select.value = names.includes(current) ? current : names[0];
  }
}

function updateModeUi() {
  updateModeSpecificControls(els.markMode.value);

  els.singleChannelRow.classList.toggle(
    "hidden",
    els.valueMode.value !== "single-channel",
  );
  els.curveSpanRow.classList.toggle("hidden", els.markMode.value !== "curve");
  els.curveTileCellsRow.classList.toggle(
    "hidden",
    els.markMode.value !== "curve" || !els.curveSpan.value.startsWith("tiled"),
  );
  els.curveModeHint.classList.toggle("hidden", els.markMode.value !== "curve");
  els.syncCurveChannelsRow.classList.toggle("hidden", els.markMode.value !== "curve");
  els.sharedCurveEditControls.classList.toggle(
    "hidden",
    els.markMode.value !== "curve" || !els.syncCurveChannels.checked,
  );
  els.sharedSmoothSeam.closest("label")?.classList.toggle(
    "hidden",
    els.markMode.value !== "curve" ||
      !els.syncCurveChannels.checked ||
      !els.sharedConnectEndpoints.checked ||
      els.curveSpan.value === "motif-pattern",
  );

  if (els.markMode.value !== "curve") {
    state.activeCurveEditTarget = null;
  } else if (els.syncCurveChannels.checked && state.activeCurveEditTarget && state.activeCurveEditTarget !== "shared") {
    state.activeCurveEditTarget = "shared";
  } else if (!els.syncCurveChannels.checked && state.activeCurveEditTarget === "shared") {
    state.activeCurveEditTarget = null;
  }

  for (const block of document.querySelectorAll(".channel-mark")) {
    block.classList.toggle(
      "hidden",
      (els.markMode.value === "curve" && els.syncCurveChannels.checked) ||
        (els.markMode.value !== "curve" && els.geometryMode.value === "shared"),
    );
  }

  for (const block of document.querySelectorAll(".curve-continuity")) {
    block.classList.toggle(
      "hidden",
      els.markMode.value !== "curve" || els.syncCurveChannels.checked,
    );
  }

  for (const block of document.querySelectorAll(".curve-pattern-controls:not(.channel-grid-controls)")) {
    block.classList.toggle("hidden", els.markMode.value !== "curve");
  }

  updateChannelSettingsTargetUi();

  for (const textarea of document.querySelectorAll("[id$='-path']")) {
    textarea.closest("label")?.classList.toggle(
      "hidden",
      els.markMode.value === "curve",
    );
  }

  for (const checkbox of document.querySelectorAll("[id$='-smoothSeam']")) {
    const key = checkbox.id.replace("-smoothSeam", "");
    checkbox.closest("label")?.classList.toggle(
      "hidden",
      els.markMode.value !== "curve" ||
        els.syncCurveChannels.checked ||
        els.curveSpan.value === "motif-pattern" ||
        !document.querySelector(`#${key}-connectEndpoints`).checked,
    );
  }

  const curveMode = els.markMode.value === "curve";
  const synchronizedCurveMode = curveMode && els.syncCurveChannels.checked;
  const sharedShapeMode = !curveMode && els.geometryMode.value === "shared";
  const showSharedGeometry = synchronizedCurveMode || sharedShapeMode;
  els.geometryMode.closest("label")?.classList.toggle("hidden", curveMode);
  els.geometryMode.closest(".panel")?.classList.toggle(
    "hidden",
    curveMode && !els.syncCurveChannels.checked,
  );
  els.sharedGeometryControls.classList.toggle(
    "hidden",
    !showSharedGeometry,
  );
  els.sharedPreset.closest("label")?.classList.toggle(
    "hidden",
    !showSharedGeometry,
  );
  els.sharedPath.closest("label")?.classList.toggle(
    "hidden",
    !showSharedGeometry || curveMode,
  );
  els.sharedCurveFile.closest("label")?.classList.toggle(
    "hidden",
    !synchronizedCurveMode,
  );

  els.sharedCurveEditButton.textContent =
    state.activeCurveEditTarget === "shared" ? "Stop Editing Shared Curve" : "Edit Shared Curve";

  els.geometryModeHint.textContent =
    els.markMode.value === "curve"
      ? els.syncCurveChannels.checked
        ? "Editing targets the shared source curve. Channel cards keep separate curve scale, tiling, stacking, rotation, offsets, sample density, and output quality."
        : "Each channel owns its own source curve plus independent curve scale, tiling, stacking, rotation, offsets, sample density, and output quality."
      : els.geometryMode.value === "shared"
      ? "One shape/curve definition is used for C, M, Y, and K. Each channel keeps its own rotation and XY offset."
      : "Each channel can use its own preset or arbitrary SVG path for the current shape/curve mode.";

  updateEffectiveChannelControlVisibility();
}

function updateModeSpecificControls(mode) {
  for (const control of document.querySelectorAll("[data-mode]")) {
    const supportedModes = control.dataset.mode.split(/\s+/u);
    control.classList.toggle("hidden", !supportedModes.includes(mode));
  }
}

function updateEffectiveChannelControlVisibility() {
  const curveMode = els.markMode.value === "curve";
  const motifCurveLayout = curveMode && els.curveSpan.value === "motif-pattern";
  const crosshatchMode = els.valueMode.value === "crosshatch-luminance";
  const deltas = readCmykDeltas();
  const channels = readBaseChannels();

  for (const control of document.querySelectorAll("[data-curve-layout]")) {
    control.classList.toggle(
      "hidden",
      !curveMode || control.dataset.curveLayout !== els.curveSpan.value,
    );
  }

  for (const control of document.querySelectorAll("[data-channel-color]")) {
    control.classList.toggle("hidden", crosshatchMode);
  }

  for (const card of document.querySelectorAll(".channel-card[data-channel-card]")) {
    const channel = channels[card.dataset.channelCard];
    card.classList.toggle("channel-disabled-settings", !channel?.enabled);
  }

  for (const control of document.querySelectorAll("[data-grid-pivot-controls]")) {
    const target = control.dataset.gridPivotControls;
    const hasEffectiveGridRotation =
      target === "cmyk"
        ? Object.values(channels).some((channel) =>
            channel.enabled &&
            Math.abs(channel.gridRotation + deltas.gridRotationDelta) > 0.0001,
          )
        : Math.abs((channels[target]?.gridRotation ?? 0) + deltas.gridRotationDelta) > 0.0001;

    control.classList.toggle("hidden", !hasEffectiveGridRotation);
  }

  els.sharedSmoothSeam.closest("label")?.classList.toggle(
    "hidden",
    !curveMode ||
      motifCurveLayout ||
      !els.syncCurveChannels.checked ||
      !els.sharedConnectEndpoints.checked,
  );
}

function updateChannelSettingsTargetUi() {
  const target = els.channelSettingsTarget.value;
  els.cmykDeltaControls.classList.toggle("inactive-settings-target", target !== "cmyk");
  for (const card of document.querySelectorAll(".channel-card[data-channel-card]")) {
    card.classList.toggle("inactive-settings-target", card.dataset.channelCard !== target);
  }
}

function updateMarkPreviews() {
  const settings = readSettings();
  const channels = readBaseChannels();
  updateSharedCurvePreview(settings);

  for (const key of Object.keys(CHANNELS)) {
    const channel = channels[key];
    const rawPath = resolveChannelPreviewPath(settings, channel);
    const d = settings.markMode === "curve" ? svgSourceToPathData(rawPath) : rawPath;
    const mount = document.querySelector(`#${key}-preview`);

    if (settings.markMode === "curve") {
      clearCurveEditor(mount);
      mount.classList.remove("curve-editor-mount");
      mount.innerHTML = renderMarkPreview({
        mode: settings.markMode,
        d,
        color: channel.color,
        rotation: channel.rotation,
      });
    } else {
      clearCurveEditor(mount);
      mount.classList.remove("curve-editor-mount");
      mount.innerHTML = renderMarkPreview({
        mode: settings.markMode,
        d,
        color: channel.color,
        rotation: channel.rotation,
      });
    }
    updateChannelEditButton(key, settings);
  }

  updateCurveEditOverlay(settings, channels);
}

function updateSharedCurvePreview(settings) {
  if (settings.markMode !== "curve" || !settings.syncCurveChannels) {
    clearCurveEditor(els.sharedCurvePreview);
    els.sharedCurvePreview.innerHTML = "";
    return;
  }

  const d = svgSourceToPathData(resolveBaseCurvePath());
  clearCurveEditor(els.sharedCurvePreview);
  els.sharedCurvePreview.classList.remove("curve-editor-mount");
  els.sharedCurvePreview.innerHTML = renderMarkPreview({
    mode: "curve",
    d,
    color: "#f8fafc",
    rotation: 0,
  });
}

function updateCurveEditOverlay(settings, channels) {
  if (settings.markMode !== "curve" || !state.activeCurveEditTarget) {
    clearCurveEditor(els.curveEditOverlayMount);
    els.curveEditOverlay.classList.add("hidden");
    return;
  }

  const target = state.activeCurveEditTarget;
  const editingShared = target === "shared" || settings.syncCurveChannels;
  const channel = editingShared ? channels.c : channels[target];
  if (!channel) {
    els.curveEditOverlay.classList.add("hidden");
    return;
  }

  const d = svgSourceToPathData(
    editingShared
      ? resolveBaseCurvePath()
      : resolveChannelPreviewPath(settings, channel),
  );
  const documentEdit = createDocumentCurveEdit(d, settings, channel);
  const editorPath = documentEdit?.path ?? d;
  els.curveEditOverlay.classList.remove("hidden");
  els.curveEditOverlayTitle.textContent = editingShared
    ? "Editing shared CMYK source curve"
    : `Editing ${CHANNELS[target].name} source curve`;

  mountCurveEditor({
    mount: els.curveEditOverlayMount,
    d: editorPath,
    color: editingShared ? "#f8fafc" : channel.color,
    rotation: editingShared ? 0 : channel.rotation,
    connectEndpoints: editingShared
      ? settings.sharedConnectEndpoints
      : channel.connectEndpoints,
    smoothSeamTangents: editingShared
      ? settings.sharedSmoothSeamTangents
      : channel.smoothSeamTangents,
    editable: true,
    normalizePath: !documentEdit,
    viewBounds: documentEdit
      ? { minX: 0, minY: 0, width: settings.outputWidth, height: settings.outputHeight }
      : null,
    editLabel: editingShared
      ? "Editing shared CMYK source curve"
      : `Editing ${CHANNELS[target].name} source curve`,
    onChange: (nextPath) => {
      const storedPath = documentEdit?.toSourcePath(nextPath) ?? nextPath;
      if (editingShared) {
        els.sharedPath.value = storedPath;
        syncAllChannelCurves(storedPath);
      } else {
        document.querySelector(`#${target}-path`).value = storedPath;
      }
      updateMarkPreviews();
      queueRender();
    },
  });
  updateCurveEditOverlayLayout();
}

function createDocumentCurveEdit(pathData, settings, channel) {
  const layout = normalizeEditCurveLayout(settings.curveSpan);
  if (usesPatternCurveEditLayout(layout, channel)) return null;

  const curve = pathDataToEditableCurve(pathData);
  if ((curve.nodes ?? []).length < 2) return null;

  const transform = createDocumentCurveTransform(curve, settings, channel, layout);
  const documentCurve = transformCurve(curve, (point) => sourceToDocumentPoint(point, transform), (vector) =>
    rotateVector(scaleVector(vector, transform.scale), transform.angle),
  );

  return {
    path: editableCurveToPathData(documentCurve, { connectEndpoints: false }),
    toSourcePath(nextDocumentPath) {
      const nextDocumentCurve = pathDataToEditableCurve(nextDocumentPath);
      const nextSourceCurve = transformCurve(
        nextDocumentCurve,
        (point) => documentToSourcePoint(point, transform),
        (vector) => scaleVector(rotateVector(vector, -transform.angle), 1 / transform.scale),
      );
      return editableCurveToPathData(nextSourceCurve, { connectEndpoints: false });
    },
  };
}

function createDocumentCurveTransform(curve, settings, channel, layout) {
  const baselineAngle = (layout.endsWith("height") ? 90 : 0) + finite(channel.rotation);
  const targetLength = artboardProjectionSpan(settings, baselineAngle) * documentCurveScaleFactor(channel);
  const nodes = curve.nodes ?? [];
  const start = nodes[0].position;
  const end = nodes.at(-1).position;
  const endpointLength = Math.hypot(end.x - start.x, end.y - start.y);
  const sourceBounds = curveBounds(curve);
  const sourceOrigin = endpointLength > 0.0001
    ? start
    : { x: sourceBounds.minX, y: sourceBounds.minY + sourceBounds.height / 2 };
  const sourceLength = endpointLength > 0.0001
    ? endpointLength
    : sourceBounds.width || endpointLength || 1;
  const scale = targetLength / sourceLength;
  const scaledCurve = transformCurve(curve, (point) => ({
    x: (point.x - sourceOrigin.x) * scale,
    y: (point.y - sourceOrigin.y) * scale,
  }), (vector) => scaleVector(vector, scale));
  const scaledBounds = curveBounds(scaledCurve);
  const centeredOffset = {
    x: -scaledBounds.minX + (settings.outputWidth - scaledBounds.width) / 2,
    y: -scaledBounds.minY + (settings.outputHeight - scaledBounds.height) / 2,
  };

  return {
    angle: baselineAngle,
    scale,
    sourceOrigin,
    centeredOffset,
    pageCenter: { x: settings.outputWidth / 2, y: settings.outputHeight / 2 },
    channelOffset: { x: finite(channel.offsetX), y: finite(channel.offsetY) },
  };
}

function sourceToDocumentPoint(point, transform) {
  const scaled = {
    x: (point.x - transform.sourceOrigin.x) * transform.scale + transform.centeredOffset.x,
    y: (point.y - transform.sourceOrigin.y) * transform.scale + transform.centeredOffset.y,
  };
  const rotated = rotatePoint(scaled, transform.pageCenter, transform.angle);
  return {
    x: rotated.x + transform.channelOffset.x,
    y: rotated.y + transform.channelOffset.y,
  };
}

function documentToSourcePoint(point, transform) {
  const unoffset = {
    x: point.x - transform.channelOffset.x,
    y: point.y - transform.channelOffset.y,
  };
  const unrotated = rotatePoint(unoffset, transform.pageCenter, -transform.angle);
  return {
    x: (unrotated.x - transform.centeredOffset.x) / transform.scale + transform.sourceOrigin.x,
    y: (unrotated.y - transform.centeredOffset.y) / transform.scale + transform.sourceOrigin.y,
  };
}

function transformCurve(curve, transformPoint, transformVector) {
  return {
    nodes: (curve.nodes ?? []).map((node) => ({
      ...node,
      position: transformPoint(node.position),
      handleIn: transformVector(node.handleIn),
      handleOut: transformVector(node.handleOut),
    })),
  };
}

function normalizeEditCurveLayout(layout) {
  if (layout === "document-width") return "full-width";
  if (layout === "document-height") return "full-height";
  if (layout === "document-fit") return "full-width";
  if (layout === "cell-chain") return "tiled-width";
  return layout || "full-width";
}

function usesPatternCurveEditLayout(layout) {
  return layout === "motif-pattern";
}

function artboardProjectionSpan(settings, angle) {
  const radians = (angle * Math.PI) / 180;
  return (
    Math.abs(settings.outputWidth * Math.cos(radians)) +
    Math.abs(settings.outputHeight * Math.sin(radians))
  );
}

function documentCurveScaleFactor(channel) {
  const curveScale = finite(channel.curveScale, 32);
  return curveScale > 0 ? curveScale / 32 : 1;
}

function rotatePoint(point, center, degrees) {
  const vector = rotateVector({ x: point.x - center.x, y: point.y - center.y }, degrees);
  return { x: center.x + vector.x, y: center.y + vector.y };
}

function rotateVector(vector, degrees) {
  const radians = (degrees * Math.PI) / 180;
  const cos = Math.cos(radians);
  const sin = Math.sin(radians);
  return {
    x: finite(vector.x) * cos - finite(vector.y) * sin,
    y: finite(vector.x) * sin + finite(vector.y) * cos,
  };
}

function scaleVector(vector, scale) {
  return {
    x: finite(vector.x) * scale,
    y: finite(vector.y) * scale,
  };
}

function finite(value, fallback = 0) {
  return Number.isFinite(Number(value)) ? Number(value) : fallback;
}

function updateChannelEditButton(key, settings) {
  const button = document.querySelector(`[data-edit-curve="${key}"]`);
  if (!button) return;

  const channelName = CHANNELS[key].name;
  if (settings.markMode !== "curve") {
    button.classList.add("hidden");
    return;
  }

  button.classList.remove("hidden");
  button.disabled = settings.syncCurveChannels;
  button.textContent = settings.syncCurveChannels
    ? "Shared Curve Preview"
    : state.activeCurveEditTarget === key
      ? `Stop Editing ${channelName}`
      : `Edit ${channelName} Curve`;
  button.closest(".channel-card")?.classList.toggle(
    "active-curve-card",
    !settings.syncCurveChannels && state.activeCurveEditTarget === key,
  );
  button.closest(".channel-card")?.classList.toggle(
    "ghost-curve-card",
    settings.markMode === "curve" &&
      (settings.syncCurveChannels || state.activeCurveEditTarget !== key),
  );
}

function resolveChannelPreviewPath(settings, channel) {
  if (settings.markMode === "curve") {
    return settings.syncCurveChannels
      ? settings.sharedPath.trim() || getPresetPath("curve", settings.sharedPreset)
      : channel.customPath.trim() ||
      settings.sharedPath.trim() ||
      getPresetPath("curve", channel.preset || settings.sharedPreset);
  }

  return settings.useSharedMark
    ? settings.sharedPath.trim() ||
      getPresetPath(settings.markMode, settings.sharedPreset)
    : channel.customPath.trim() ||
      getPresetPath(settings.markMode, channel.preset);
}

function resolveBaseCurvePath() {
  return els.sharedPath.value.trim() || getPresetPath("curve", els.sharedPreset.value);
}

function syncAllChannelCurves(path, sourceKey = null) {
  const nextPath = path.trim();
  if (!nextPath) return;

  for (const key of Object.keys(CHANNELS)) {
    if (key === sourceKey) continue;
    document.querySelector(`#${key}-path`).value = nextPath;
  }
}

function applyAspectFromEditedDimension(dimension) {
  if (!state.source || !els.preserveAspect.checked) return;

  const locked = aspectLockedDimensions({
    width: readNumber(els.outputWidth, 900),
    height: readNumber(els.outputHeight, 600),
    sourceWidth: state.source.width,
    sourceHeight: state.source.height,
    editedDimension: dimension,
  });

  setControlValue(els.outputWidth, locked.width);
  setControlValue(els.outputHeight, locked.height);
}

function updatePreviewScale() {
  const svg = els.previewMount.querySelector("svg");
  if (!svg) return;

  if (!state.fitPreview) {
    svg.style.width = "";
    svg.style.maxWidth = "none";
    updateInfoOverlayLayout();
    updateCurveEditOverlayLayout();
    return;
  }

  const viewBox = svg.viewBox?.baseVal;
  const svgWidth = viewBox?.width || readNumber({ value: svg.getAttribute("width") }, 0);
  const svgHeight = viewBox?.height || readNumber({ value: svg.getAttribute("height") }, 0);
  if (!svgWidth || !svgHeight) return;

  const frame = els.previewFrame.getBoundingClientRect();
  const availableWidth = Math.max(1, frame.width - 48);
  const availableHeight = Math.max(1, frame.height - 48);
  const scale = Math.min(availableWidth / svgWidth, availableHeight / svgHeight);
  const fittedWidth = Math.max(svgWidth, svgWidth * scale);

  svg.style.width = `${Math.round(fittedWidth)}px`;
  svg.style.maxWidth = "none";
  updateInfoOverlayLayout();
  updateCurveEditOverlayLayout();
}

function updateFitPreviewButton() {
  els.fitPreviewButton.textContent = state.fitPreview ? "Actual size" : "Fit preview";
}

function readNumber(input, fallback) {
  const value = Number.parseFloat(input.value);
  return Number.isFinite(value) ? value : fallback;
}

function clamp(value, min, max) {
  return Math.min(max, Math.max(min, value));
}

function clampInteger(value, min, max) {
  return Math.round(clamp(value, min, max));
}

function sourceBaseName() {
  return (state.source?.fileName || "halftone")
    .replace(/\.[^.]+$/, "")
    .replace(/[^a-z0-9_-]+/gi, "-")
    .replace(/^-+|-+$/g, "")
    .toLowerCase();
}

function sanitizeFileName(value) {
  return String(value || "toniator-preset")
    .replace(/\.[^.]+$/, "")
    .replace(/[^a-z0-9_-]+/gi, "-")
    .replace(/^-+|-+$/g, "")
    .toLowerCase() || "toniator-preset";
}

function describeChannelResolutions(channels, baseLongEdgeCells) {
  return Object.entries(channels)
    .filter(([, channel]) => channel.enabled)
    .map(([key, channel]) => {
      const cells = Math.max(
        2,
        Math.round(baseLongEdgeCells * Math.max(0.05, channel.resolutionScale)),
      );
      return `${key.toUpperCase()} ${cells}`;
    })
    .join(" · ");
}

function title(value) {
  return value.replace(/[-_]+/g, " ").replace(/\b\w/g, (c) => c.toUpperCase());
}
