export const shapePresets = {
  circle:
    "M 0 -0.5 C 0.276 -0.5 0.5 -0.276 0.5 0 C 0.5 0.276 0.276 0.5 0 0.5 C -0.276 0.5 -0.5 0.276 -0.5 0 C -0.5 -0.276 -0.276 -0.5 0 -0.5 Z",
  rectangle: "M -0.45 -0.45 L 0.45 -0.45 L 0.45 0.45 L -0.45 0.45 Z",
  triangle: "M 0 -0.52 L 0.5 0.4 L -0.5 0.4 Z",
  pentagon:
    "M 0 -0.5 L 0.4755 -0.1545 L 0.2939 0.4045 L -0.2939 0.4045 L -0.4755 -0.1545 Z",
  hexagon:
    "M 0.433 -0.25 L 0.433 0.25 L 0 0.5 L -0.433 0.25 L -0.433 -0.25 L 0 -0.5 Z",
};

export const curvePresets = {
  line: "M -0.45 0 L 0.45 0",
  slash: "M -0.42 0.42 L 0.42 -0.42",
  arc: "M -0.45 0.18 C -0.25 -0.35 0.25 -0.35 0.45 0.18",
  wave: "M -0.5 0 C -0.32 -0.4 -0.18 -0.4 0 0 C 0.18 0.4 0.32 0.4 0.5 0",
  curve: "M -0.45 0.32 C -0.22 -0.38 0.22 -0.38 0.45 0.32",
  v: "M -0.42 -0.3 L 0 0.34 L 0.42 -0.3",
  loop:
    "M -0.45 0 C -0.35 -0.35 -0.05 -0.35 0 0 C 0.05 0.35 0.35 0.35 0.45 0",
};

export function getPresetNames(mode) {
  return Object.keys(mode === "curve" ? curvePresets : shapePresets);
}

export function getPresetPath(mode, name) {
  const presets = mode === "curve" ? curvePresets : shapePresets;
  return presets[name] || Object.values(presets)[0];
}
