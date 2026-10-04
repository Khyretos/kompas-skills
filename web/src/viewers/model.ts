// 3D and animation viewer (three.js, MIT), loaded only when a viewer opens. Models come
// from the server as their original files; the textures and .mtl/.bin files they name are
// found in the same pack by `nearUrl`. Everything is freed when the viewer closes.
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { FBXLoader } from "three/examples/jsm/loaders/FBXLoader.js";
import { OBJLoader } from "three/examples/jsm/loaders/OBJLoader.js";
import { MTLLoader } from "three/examples/jsm/loaders/MTLLoader.js";
import { GLTFLoader } from "three/examples/jsm/loaders/GLTFLoader.js";
import { BVHLoader } from "three/examples/jsm/loaders/BVHLoader.js";
import { TGALoader } from "three/examples/jsm/loaders/TGALoader.js";
import { STLLoader } from "three/examples/jsm/loaders/STLLoader.js";
import { PLYLoader } from "three/examples/jsm/loaders/PLYLoader.js";
import { ColladaLoader } from "three/examples/jsm/loaders/ColladaLoader.js";

export const MODEL_EXTS = ["fbx", "obj", "gltf", "glb", "bvh", "stl", "ply", "dae"];

export interface ModelSource {
  url: string; // the model file
  near: (name: string) => string; // a file the model names, from the same pack
  ext: string;
  name: string;
  /** Textures of the same pack, for models that don't name theirs (Synty atlases). */
  textures: () => Promise<{ name: string; url: string }[]>;
}

export interface ModelInfo { meshes: number; triangles: number; bones: number; clips: string[] }

/** Builds the viewer in `el`. Resolves once the model is shown; the returned function
 *  frees the GPU memory and the DOM. */
export async function mountModel(el: HTMLElement, src: ModelSource, onInfo?: (i: ModelInfo) => void): Promise<() => void> {
  const abort = new AbortController();
  const on = <K extends keyof HTMLElementEventMap>(t: EventTarget, type: K | string, f: (e: Event) => void) =>
    t.addEventListener(type, f, { signal: abort.signal });
  el.replaceChildren();
  const tools = Object.assign(document.createElement("div"), { className: "model-tools" });
  const stage = Object.assign(document.createElement("div"), { className: "model-stage" });
  stage.setAttribute("aria-label", "3D view: drag to turn, scroll to zoom, right-drag to move");
  stage.tabIndex = 0;
  const status = Object.assign(document.createElement("p"), { className: "model-status muted", textContent: "Loading the model…" });
  status.setAttribute("aria-live", "polite");
  el.append(tools, stage, status);

  let renderer: THREE.WebGLRenderer;
  try {
    renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true, powerPreference: "low-power" });
  } catch {
    status.textContent = "This browser can't show 3D (WebGL is off).";
    return () => { abort.abort(); el.replaceChildren(); };
  }
  renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  stage.append(renderer.domElement);

  const scene = new THREE.Scene();
  const camera = new THREE.PerspectiveCamera(45, 1, 0.01, 5000);
  scene.add(new THREE.HemisphereLight(0xf4eefc, 0x2c1f3f, 1.6));
  const sun = new THREE.DirectionalLight(0xffffff, 2.2);
  sun.position.set(3, 6, 4);
  scene.add(sun);
  const controls = new OrbitControls(camera, renderer.domElement);
  controls.enableDamping = true;

  const manager = new THREE.LoadingManager();
  // Textures and side files the model names: look them up in the same pack.
  manager.setURLModifier((url) => {
    if (url === src.url || url.startsWith("blob:") || url.startsWith("data:")) return url;
    const marker = src.url.slice(0, src.url.lastIndexOf("/") + 1);
    const name = decodeURIComponent(url.startsWith(marker) ? url.slice(marker.length) : url);
    return src.near(name);
  });
  manager.addHandler(/\.tga$/i, new TGALoader(manager));
  const missing = new Set<string>();
  manager.onError = (url) => missing.add(url);

  let root: THREE.Object3D;
  let clips: THREE.AnimationClip[] = [];
  try {
    const progress = (e: ProgressEvent) => {
      if (e.total) status.textContent = `Loading the model… ${Math.round(e.loaded / 1048576)} of ${Math.round(e.total / 1048576)} MB`;
    };
    ({ root, clips } = await load(src, manager, progress));
  } catch (e) {
    renderer.dispose();
    renderer.forceContextLoss();
    status.textContent = e instanceof Error && e.message ? `This file couldn't be shown: ${e.message}` : "This file couldn't be shown.";
    return () => { abort.abort(); el.replaceChildren(); };
  }
  scene.add(root);

  // Count what's in it; show the bones when there's no mesh (an animation-only file).
  let meshes = 0, triangles = 0, bones = 0;
  root.traverse((o) => {
    if ((o as THREE.Mesh).isMesh) {
      meshes++;
      const g = (o as THREE.Mesh).geometry;
      triangles += g.index ? g.index.count / 3 : (g.attributes.position?.count ?? 0) / 3;
    }
    if ((o as THREE.Bone).isBone) bones++;
  });
  let skeleton: THREE.SkeletonHelper | undefined;
  if (bones && (!meshes || src.ext === "bvh")) {
    skeleton = new THREE.SkeletonHelper(root);
    skeleton.setColors(new THREE.Color(0xf3941f), new THREE.Color(0xcca9ff)); // Kreative Kompas orange to lilac
    scene.add(skeleton);
  }

  // Frame it: centre on the ground, camera at a three-quarter view. A skeleton's helper has
  // no size until it first renders, so bones are measured from their world positions.
  root.updateMatrixWorld(true);
  const box = new THREE.Box3();
  if (skeleton) root.traverse((o) => { if ((o as THREE.Bone).isBone) box.expandByPoint(o.getWorldPosition(new THREE.Vector3())); });
  else box.setFromObject(root);
  if (box.isEmpty()) box.set(new THREE.Vector3(-1, 0, -1), new THREE.Vector3(1, 2, 1));
  const size = box.getSize(new THREE.Vector3());
  const centre = box.getCenter(new THREE.Vector3());
  const radius = Math.max(size.length() / 2, 0.01);
  const grid = new THREE.GridHelper(radius * 4, 20, 0x5c398e, 0x2c1f3f);
  grid.position.y = box.min.y;
  scene.add(grid);
  const fit = () => {
    camera.near = radius / 100;
    camera.far = radius * 100;
    camera.position.copy(centre).add(new THREE.Vector3(radius * 1.6, radius * 1.1, radius * 2.0));
    camera.updateProjectionMatrix();
    controls.target.copy(centre);
    controls.update();
  };
  fit();

  // Animation.
  const mixer = new THREE.AnimationMixer(root);
  let action: THREE.AnimationAction | undefined;
  let playing = true;
  const clock = new THREE.Clock();

  // Toolbar.
  const button = (label: string, title: string) => {
    const b = Object.assign(document.createElement("button"), { type: "button", className: "btn small", textContent: label, title });
    tools.append(b);
    return b;
  };
  on(button("Fit", "Frame the model again"), "click", fit);
  const wire = Object.assign(document.createElement("label"), { className: "asset-check" });
  const wireBox = Object.assign(document.createElement("input"), { type: "checkbox" });
  wire.append(wireBox, " Wireframe");
  tools.append(wire);
  on(wireBox, "change", () => root.traverse((o) => {
    const m = (o as THREE.Mesh).material;
    for (const x of Array.isArray(m) ? m : m ? [m] : []) (x as THREE.MeshStandardMaterial).wireframe = wireBox.checked;
  }));

  // Synty and other packs keep wind or masks in the vertex colours: shown, they tint a
  // textured model almost black. Off by default when the model has a texture.
  const materials = () => {
    const out: THREE.Material[] = [];
    root.traverse((o) => { const m = (o as THREE.Mesh).material; if (m) out.push(...(Array.isArray(m) ? m : [m])); });
    return out;
  };
  if (materials().some((m) => m.vertexColors)) {
    const vc = Object.assign(document.createElement("label"), { className: "asset-check" });
    const vcBox = Object.assign(document.createElement("input"), { type: "checkbox", className: "model-vc" });
    vc.append(vcBox, " Vertex colours");
    tools.append(vc);
    const setVc = () => { for (const m of materials()) { m.vertexColors = vcBox.checked; m.needsUpdate = true; } };
    vcBox.checked = !materials().some((m) => (m as THREE.MeshStandardMaterial).map);
    setVc();
    on(vcBox, "change", setVc);
  }

  // A pack texture for models that name none (or whose textures weren't found).
  let untextured = meshes > 0;
  root.traverse((o) => {
    const m = (o as THREE.Mesh).material;
    for (const x of Array.isArray(m) ? m : m ? [m] : []) if ((x as THREE.MeshStandardMaterial).map) untextured = false;
  });
  let picked: THREE.Texture | undefined;
  if (meshes > 0) {
    const pick = Object.assign(document.createElement("select"), { className: "model-texture" });
    pick.setAttribute("aria-label", "Texture from this pack");
    pick.append(new Option(untextured ? "No texture: pick one from the pack…" : "Model's own textures", ""));
    tools.append(pick);
    void src.textures().then((list) => {
      for (const t of list) pick.append(new Option(t.name, t.url));
      if (!list.length) pick.disabled = true;
    });
    const own = new Map<THREE.Material, THREE.Texture | null>();
    on(pick, "change", () => {
      picked?.dispose();
      picked = undefined;
      const apply = (map: THREE.Texture | null) => root.traverse((o) => {
        const m = (o as THREE.Mesh).material;
        for (const x of Array.isArray(m) ? m : m ? [m] : []) {
          const mat = x as THREE.MeshStandardMaterial;
          if (!own.has(mat)) own.set(mat, mat.map ?? null);
          mat.map = map ?? own.get(mat) ?? null;
          if (map) mat.color?.set(0xffffff);
          mat.needsUpdate = true;
        }
      });
      if (!pick.value) { apply(null); return; }
      const loader = /\.tga$/i.test(pick.selectedOptions[0]?.text ?? "") ? new TGALoader() : new THREE.TextureLoader();
      loader.load(pick.value, (tex) => {
        tex.colorSpace = THREE.SRGBColorSpace;
        if (src.ext !== "fbx" && src.ext !== "obj") tex.flipY = false;
        picked = tex;
        apply(tex);
      });
    });
  }

  if (clips.length) {
    const sel = Object.assign(document.createElement("select"), { className: "model-clip" });
    sel.setAttribute("aria-label", "Animation");
    clips.forEach((c, i) => sel.append(new Option(`${c.name || `Clip ${i + 1}`} (${c.duration.toFixed(1)} s)`, String(i))));
    const play = button("Pause", "Play or pause");
    const scrub = Object.assign(document.createElement("input"), { type: "range", className: "model-scrub", min: "0", max: "1000", value: "0" });
    scrub.setAttribute("aria-label", "Animation time");
    const speed = Object.assign(document.createElement("select"), { className: "model-speed" });
    speed.setAttribute("aria-label", "Speed");
    for (const s of ["0.25", "0.5", "1", "1.5", "2"]) speed.append(new Option(`${s}×`, s, s === "1", s === "1"));
    tools.prepend(sel);
    tools.append(scrub, speed);
    const start = (i: number) => {
      action?.stop();
      action = mixer.clipAction(clips[i]);
      action.play();
      playing = true;
      play.textContent = "Pause";
    };
    start(0);
    on(sel, "change", () => start(Number(sel.value)));
    on(play, "click", () => { playing = !playing; play.textContent = playing ? "Pause" : "Play"; });
    on(scrub, "input", () => {
      if (!action) return;
      playing = false;
      play.textContent = "Play";
      action.time = (Number(scrub.value) / 1000) * action.getClip().duration;
      mixer.update(0);
    });
    on(speed, "change", () => { mixer.timeScale = Number(speed.value); });
    const tick = () => {
      if (action && playing) scrub.value = String(Math.round((action.time / Math.max(action.getClip().duration, 0.001)) * 1000));
    };
    renderer.setAnimationLoop(() => frame(tick));
  } else {
    renderer.setAnimationLoop(() => frame());
  }

  function frame(extra?: () => void): void {
    const dt = clock.getDelta();
    if (playing) mixer.update(dt);
    controls.update();
    extra?.();
    renderer.render(scene, camera);
  }

  const resize = () => {
    const w = stage.clientWidth || 1, h = stage.clientHeight || 1;
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
  };
  const ro = new ResizeObserver(resize);
  ro.observe(stage);
  resize();

  const info: ModelInfo = { meshes, triangles: Math.round(triangles), bones, clips: clips.map((c) => c.name) };
  status.textContent = [
    meshes ? `${meshes} mesh${meshes === 1 ? "" : "es"}, ${info.triangles.toLocaleString("en")} triangles` : "",
    bones ? `${bones} bones` : "",
    clips.length ? `${clips.length} animation${clips.length === 1 ? "" : "s"}` : "",
    missing.size ? `${missing.size} referenced file${missing.size === 1 ? "" : "s"} not found in the pack` : "",
  ].filter(Boolean).join(" · ");
  stage.dataset.state = "ready";
  onInfo?.(info);

  return () => {
    abort.abort();
    ro.disconnect();
    renderer.setAnimationLoop(null);
    mixer.stopAllAction();
    controls.dispose();
    picked?.dispose();
    scene.traverse((o) => {
      const mesh = o as THREE.Mesh;
      mesh.geometry?.dispose();
      const m = mesh.material;
      for (const x of Array.isArray(m) ? m : m ? [m] : []) {
        for (const v of Object.values(x)) if (v instanceof THREE.Texture) v.dispose();
        x.dispose();
      }
    });
    renderer.dispose();
    renderer.forceContextLoss();
    el.replaceChildren();
  };
}

async function load(src: ModelSource, manager: THREE.LoadingManager, progress: (e: ProgressEvent) => void): Promise<{ root: THREE.Object3D; clips: THREE.AnimationClip[] }> {
  const material = () => new THREE.MeshStandardMaterial({ color: 0xcca9ff, roughness: 0.8, metalness: 0.05 });
  switch (src.ext) {
    case "fbx": {
      const g = await new FBXLoader(manager).loadAsync(src.url, progress);
      return { root: g, clips: g.animations };
    }
    case "gltf":
    case "glb": {
      const g = await new GLTFLoader(manager).loadAsync(src.url, progress);
      return { root: g.scene, clips: g.animations };
    }
    case "obj": {
      const loader = new OBJLoader(manager);
      // The .mtl next to it, if the pack has one (OBJ files name theirs inside, but Synty's
      // usually point at a path on the artist's PC).
      let hasMtl = false;
      try {
        const mtl = await new MTLLoader(manager).loadAsync(src.near(src.name.replace(/\.obj$/i, ".mtl")));
        mtl.preload();
        loader.setMaterials(mtl);
        hasMtl = true;
      } catch { /* no .mtl: the viewer's own material */ }
      const g = await loader.loadAsync(src.url, progress);
      if (!hasMtl) g.traverse((o) => { if ((o as THREE.Mesh).isMesh) (o as THREE.Mesh).material = material(); });
      return { root: g, clips: [] };
    }
    case "bvh": {
      const r = await new BVHLoader(manager).loadAsync(src.url, progress);
      const root = new THREE.Group();
      root.add(r.skeleton.bones[0]);
      return { root, clips: [r.clip] };
    }
    case "stl": {
      const geo = await new STLLoader(manager).loadAsync(src.url, progress);
      geo.computeVertexNormals();
      return { root: new THREE.Mesh(geo, material()), clips: [] };
    }
    case "ply": {
      const geo = await new PLYLoader(manager).loadAsync(src.url, progress);
      geo.computeVertexNormals();
      return { root: new THREE.Mesh(geo, material()), clips: [] };
    }
    case "dae": {
      const c = await new ColladaLoader(manager).loadAsync(src.url, progress);
      if (!c) throw new Error("empty Collada file");
      return { root: c.scene, clips: c.scene.animations ?? [] };
    }
    default:
      throw new Error(`no in-browser viewer for .${src.ext}`);
  }
}
