export function aspectLockedDimensions({
  width,
  height,
  sourceWidth,
  sourceHeight,
  editedDimension,
}) {
  const aspect = sourceWidth / sourceHeight;
  if (!Number.isFinite(aspect) || aspect <= 0) {
    return { width, height };
  }

  if (editedDimension === "height") {
    return {
      width: Math.max(1, Math.round(height * aspect)),
      height,
    };
  }

  return {
    width,
    height: Math.max(1, Math.round(width / aspect)),
  };
}
