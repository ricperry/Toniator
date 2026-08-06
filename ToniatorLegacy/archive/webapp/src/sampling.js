export function calculateGrid(source, outputWidth, outputHeight, longEdgeCells) {
  const aspect = outputWidth / outputHeight;
  const safeLongEdge = Math.max(2, Math.round(longEdgeCells));

  let cols;
  let rows;
  if (aspect >= 1) {
    cols = safeLongEdge;
    rows = Math.max(1, Math.round(safeLongEdge / aspect));
  } else {
    rows = safeLongEdge;
    cols = Math.max(1, Math.round(safeLongEdge * aspect));
  }

  return {
    cols,
    rows,
    cellWidth: outputWidth / cols,
    cellHeight: outputHeight / rows,
    sourceAspect: source ? source.width / source.height : aspect,
  };
}

export function sampleImage(source, cols, rows) {
  const canvas = document.createElement("canvas");
  canvas.width = cols;
  canvas.height = rows;
  const ctx = canvas.getContext("2d", { willReadFrequently: true });

  ctx.clearRect(0, 0, cols, rows);
  ctx.drawImage(source.image, 0, 0, cols, rows);

  const imageData = ctx.getImageData(0, 0, cols, rows);
  const samples = new Array(cols * rows);

  for (let i = 0; i < samples.length; i += 1) {
    const offset = i * 4;
    samples[i] = [
      imageData.data[offset],
      imageData.data[offset + 1],
      imageData.data[offset + 2],
      imageData.data[offset + 3],
    ];
  }

  return samples;
}
