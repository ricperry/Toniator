export async function loadSourceFile(file) {
  if (!file) {
    throw new Error("No file selected.");
  }

  const dataUrl = await readAsDataUrl(file);
  const image = new Image();
  image.decoding = "async";

  if (file.type === "image/svg+xml" || file.name.toLowerCase().endsWith(".svg")) {
    const text = await file.text();
    const svgSize = getSvgIntrinsicSize(text);
    image.src = dataUrl;
    await image.decode();
    return {
      fileName: file.name,
      type: "svg",
      dataUrl,
      image,
      width: svgSize.width || image.naturalWidth || 800,
      height: svgSize.height || image.naturalHeight || 600,
    };
  }

  image.src = dataUrl;
  await image.decode();
  return {
    fileName: file.name,
    type: "raster",
    dataUrl,
    image,
    width: image.naturalWidth,
    height: image.naturalHeight,
  };
}

export async function createSampleSource() {
  const width = 900;
  const height = 620;
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}">
    <defs>
      <linearGradient id="warm" x1="0" y1="0" x2="1" y2="1">
        <stop offset="0" stop-color="#ffcf33"/>
        <stop offset="0.48" stop-color="#ec008c"/>
        <stop offset="1" stop-color="#0047ff"/>
      </linearGradient>
      <radialGradient id="cool" cx="42%" cy="40%" r="70%">
        <stop offset="0" stop-color="#ffffff"/>
        <stop offset="0.45" stop-color="#00aeef"/>
        <stop offset="1" stop-color="#08111f"/>
      </radialGradient>
    </defs>
    <rect width="100%" height="100%" fill="url(#warm)"/>
    <circle cx="320" cy="290" r="210" fill="url(#cool)" opacity="0.9"/>
    <rect x="515" y="110" width="250" height="330" rx="42" fill="#101114" opacity="0.72"/>
    <path d="M95 505 C230 390 330 645 490 505 S720 420 820 540" fill="none" stroke="#fff" stroke-width="54" stroke-linecap="round" opacity="0.82"/>
    <text x="548" y="300" font-family="Arial, sans-serif" font-size="86" font-weight="800" fill="#fff">T</text>
  </svg>`;
  const dataUrl = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
  const image = new Image();
  image.decoding = "async";
  image.src = dataUrl;
  await image.decode();

  return {
    fileName: "built-in-sample.svg",
    type: "svg",
    dataUrl,
    image,
    width,
    height,
  };
}

function readAsDataUrl(file) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
}

function getSvgIntrinsicSize(text) {
  const parser = new DOMParser();
  const doc = parser.parseFromString(text, "image/svg+xml");
  const svg = doc.documentElement;

  if (!svg || svg.nodeName.toLowerCase() !== "svg") {
    return { width: 0, height: 0 };
  }

  const width = parseSvgLength(svg.getAttribute("width"));
  const height = parseSvgLength(svg.getAttribute("height"));
  if (width && height) {
    return { width, height };
  }

  const viewBox = svg.getAttribute("viewBox");
  if (viewBox) {
    const [, , vbWidth, vbHeight] = viewBox.split(/[\s,]+/).map(Number);
    if (vbWidth > 0 && vbHeight > 0) {
      return { width: vbWidth, height: vbHeight };
    }
  }

  return { width: width || 0, height: height || 0 };
}

function parseSvgLength(value) {
  if (!value) return 0;
  const parsed = Number.parseFloat(value);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : 0;
}
