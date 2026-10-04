// Font viewer: the FontFace API on the original file. Drafted by the local Coder; Claude fixed
// the order (sizes were set before the lines existed) and the size label.
/** Shows a font file: sample text, sizes and a glyph grid. Returns a function that removes everything it added (DOM, the FontFace from document.fonts). */
export async function mountFont(el: HTMLElement, url: string, fileName: string): Promise<() => void> {
  const res = await fetch(url, { credentials: "same-origin" });
  if (!res.ok) throw new Error(`The font couldn't be loaded (${res.status}).`);
  const buffer = await res.arrayBuffer();
  const family = `kk-view-${Math.random().toString(36).slice(2, 8)}`;
  const face = new FontFace(family, buffer);
  try { await face.load(); } catch { throw new Error("This font file can't be read by the browser."); }
  document.fonts.add(face);
  el.replaceChildren();
  
  const tools = document.createElement("div");
  tools.className = "font-tools";
  const inputSample = document.createElement("input");
  inputSample.className = "font-sample";
  inputSample.type = "text";
  inputSample.value = "The quick brown fox jumps over the lazy dog 0123456789";
  inputSample.setAttribute("aria-label", "Sample text");
  inputSample.setAttribute("maxlength", "200");
  
  const labelSize = document.createElement("label");
  labelSize.textContent = "Size";
  const inputSize = document.createElement("input");
  inputSize.className = "font-size";
  inputSize.type = "range";
  inputSize.min = "12";
  inputSize.max = "120";
  inputSize.value = "40";
  inputSize.setAttribute("aria-label", "Size");
  
  const outputSize = document.createElement("output");
  outputSize.className = "font-size-value";
  outputSize.textContent = "40 px";
  
  labelSize.append(" ", inputSize, " ", outputSize);
  tools.append(inputSample, labelSize);
  
  const samples = document.createElement("div");
  samples.className = "font-samples";
  const lines: HTMLElement[] = [];
  const updateSamples = (size: number) => {
    const base = size * 0.6;
    const small = Math.max(10, size * 0.35);
    lines[0].style.fontSize = `${size}px`;
    lines[1].style.fontSize = `${base}px`;
    lines[2].style.fontSize = `${small}px`;
  };
  for (let i = 0; i < 3; i++) {
    const p = document.createElement("p");
    p.className = "font-line";
    p.style.fontFamily = `"${family}", sans-serif`;
    p.textContent = inputSample.value;
    samples.appendChild(p);
    lines.push(p);
  }
  
  updateSamples(Number(inputSize.value));

  const h3 = document.createElement("h3");
  h3.className = "font-label";
  h3.textContent = "Characters";
  
  const glyphsDiv = document.createElement("div");
  glyphsDiv.className = "font-glyphs";
  const chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!?.,:;'\"()[]{}&@#%*+-=/\\<>_~$€£¥áéíóúñüÁÉÍÓÚÑÜçÇ¿¡ßøå".split("");
  const glyphCount = chars.length;
  
  for (const char of chars) {
    const span = document.createElement("span");
    span.className = "font-glyph";
    span.style.fontFamily = `"${family}", sans-serif`;
    span.style.fontSize = "28px";
    span.textContent = char;
    const code = char.charCodeAt(0).toString(16).toUpperCase().padStart(4, "0");
    span.title = `U+${code}`;
    glyphsDiv.appendChild(span);
  }
  
  const meta = document.createElement("p");
  meta.className = "font-meta";
  meta.textContent = `${fileName} · ${glyphCount} characters shown`;
  
  el.append(tools, samples, h3, glyphsDiv, meta);
  
  inputSample.addEventListener("input", () => {
    lines.forEach(l => l.textContent = inputSample.value);
  });
  
  inputSize.addEventListener("input", () => {
    const val = Number(inputSize.value);
    outputSize.textContent = `${val} px`;
    updateSamples(val);
  });
  
  return () => { document.fonts.delete(face); el.replaceChildren(); };
}
