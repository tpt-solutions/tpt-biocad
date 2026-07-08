// 3D Viewport — WebGL/Three.js scene manager

import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import type { ViewportState } from '../types';

export class Viewport3D {
  private scene: THREE.Scene;
  private camera: THREE.PerspectiveCamera;
  private renderer: THREE.WebGLRenderer;
  private controls: OrbitControls;
  private meshGroup: THREE.Group;
  private toolpathGroup: THREE.Group;
  private slumpingGroup: THREE.Group;
  private gridHelper: THREE.GridHelper;
  private axesHelper: THREE.AxesHelper;
  private ambientLight: THREE.AmbientLight;
  private directionalLight: THREE.DirectionalLight;
  private animationId: number | null = null;
  private _state: ViewportState;
  private stateListeners: Array<(state: ViewportState) => void> = [];

  constructor(container: HTMLElement) {
    this._state = {
      showMesh: true,
      showToolpath: true,
      showSlumping: false,
      currentLayer: 0,
      totalLayers: 0,
      animationSpeed: 1,
      isPlaying: false,
    };

    this.scene = new THREE.Scene();
    this.scene.background = new THREE.Color(0x1a1a2e);

    // Camera
    const rect = container.getBoundingClientRect();
    this.camera = new THREE.PerspectiveCamera(45, rect.width / rect.height, 0.1, 1000);
    this.camera.position.set(20, 20, 20);
    this.camera.lookAt(0, 0, 0);

    // Renderer
    this.renderer = new THREE.WebGLRenderer({ antialias: true });
    this.renderer.setSize(rect.width, rect.height);
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    this.renderer.shadowMap.enabled = true;
    container.appendChild(this.renderer.domElement);

    // Controls
    this.controls = new OrbitControls(this.camera, this.renderer.domElement);
    this.controls.enableDamping = true;
    this.controls.dampingFactor = 0.1;
    this.controls.target.set(0, 0, 0);
    this.controls.update();

    // Lights
    this.ambientLight = new THREE.AmbientLight(0x404060, 0.6);
    this.scene.add(this.ambientLight);

    this.directionalLight = new THREE.DirectionalLight(0xffffff, 1.0);
    this.directionalLight.position.set(10, 20, 10);
    this.directionalLight.castShadow = true;
    this.scene.add(this.directionalLight);

    const fillLight = new THREE.DirectionalLight(0x4488ff, 0.4);
    fillLight.position.set(-10, 10, -10);
    this.scene.add(fillLight);

    // Helpers
    this.gridHelper = new THREE.GridHelper(50, 20, 0x00d4ff, 0x336699);
    this.scene.add(this.gridHelper);

    this.axesHelper = new THREE.AxesHelper(10);
    this.scene.add(this.axesHelper);

    // Groups
    this.meshGroup = new THREE.Group();
    this.scene.add(this.meshGroup);

    this.toolpathGroup = new THREE.Group();
    this.scene.add(this.toolpathGroup);

    this.slumpingGroup = new THREE.Group();
    this.scene.add(this.slumpingGroup);

    // Resize handling
    const resizeObserver = new ResizeObserver(() => {
      const r = container.getBoundingClientRect();
      this.camera.aspect = r.width / r.height;
      this.camera.updateProjectionMatrix();
      this.renderer.setSize(r.width, r.height);
    });
    resizeObserver.observe(container);

    // Start render loop
    this.startLoop();
  }

  get state(): ViewportState {
    return { ...this._state };
  }

  set state(s: ViewportState) {
    this._state = { ...s };
    this.updateVisibility();
    for (const listener of this.stateListeners) listener(this._state);
  }

  onStateChange(listener: (state: ViewportState) => void): () => void {
    this.stateListeners.push(listener);
    return () => {
      const idx = this.stateListeners.indexOf(listener);
      if (idx >= 0) this.stateListeners.splice(idx, 1);
    };
  }

  private updateVisibility() {
    this.meshGroup.visible = this._state.showMesh;
    this.toolpathGroup.visible = this._state.showToolpath;
    this.slumpingGroup.visible = this._state.showSlumping;
  }

  // --- Mesh loading ---

  loadMesh(vertices: Float32Array, indices: Uint32Array | null, normals: Float32Array | null) {
    this.clearMesh();

    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute('position', new THREE.BufferAttribute(vertices, 3));

    if (indices) {
      geometry.setIndex(new THREE.BufferAttribute(indices, 1));
    }

    if (normals) {
      geometry.setAttribute('normal', new THREE.BufferAttribute(normals, 3));
    } else {
      geometry.computeVertexNormals();
    }

    const material = new THREE.MeshPhysicalMaterial({
      color: 0x4488cc,
      metalness: 0.1,
      roughness: 0.6,
      transparent: true,
      opacity: 0.85,
      wireframe: false,
      side: THREE.DoubleSide,
    });

    const mesh = new THREE.Mesh(geometry, material);
    mesh.castShadow = true;
    mesh.receiveShadow = true;
    this.meshGroup.add(mesh);

    this.fitCamera();
  }

  loadMeshFromArrays(vertices: number[][], triangles: number[][]) {
    const verts = new Float32Array(vertices.flat());
    const idx = new Uint32Array(triangles.flat());
    this.loadMesh(verts, idx, null);
  }

  clearMesh() {
    while (this.meshGroup.children.length > 0) {
      const child = this.meshGroup.children[0];
      if (child instanceof THREE.Mesh) {
        child.geometry.dispose();
        (child.material as THREE.Material).dispose();
      }
      this.meshGroup.remove(child);
    }
  }

  // --- Toolpath loading ---

  loadToolpathLayer(
    points: Array<{ x: number; y: number; z: number }>,
    layerIndex: number,
    totalLayers: number
  ) {
    if (points.length < 2) return;

    const positions: number[] = [];
    const colors: number[] = [];

    // Color gradient: blue → cyan → green → yellow → red per layer
    const hue = (layerIndex / Math.max(totalLayers, 1)) * 0.66;
    const color = new THREE.Color().setHSL(hue, 1.0, 0.5);

    for (let i = 0; i < points.length - 1; i++) {
      const p = points[i];
      const q = points[i + 1];
      positions.push(p.x, p.y, p.z, q.x, q.y, q.z);
      colors.push(color.r, color.g, color.b, color.r, color.g, color.b);
    }

    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3));
    geometry.setAttribute('color', new THREE.Float32BufferAttribute(colors, 3));

    const material = new THREE.LineBasicMaterial({
      vertexColors: true,
      linewidth: 2,
    });

    const line = new THREE.LineSegments(geometry, material);
    line.userData = { layer: layerIndex };
    line.visible = layerIndex <= this._state.currentLayer;
    this.toolpathGroup.add(line);
  }

  clearToolpath() {
    while (this.toolpathGroup.children.length > 0) {
      const child = this.toolpathGroup.children[0];
      if (child instanceof THREE.LineSegments || child instanceof THREE.Line) {
        child.geometry.dispose();
        (child.material as THREE.Material).dispose();
      }
      this.toolpathGroup.remove(child);
    }
  }

  setCurrentLayer(layer: number) {
    this._state.currentLayer = layer;
    for (const child of this.toolpathGroup.children) {
      const l = (child.userData as { layer?: number }).layer ?? Infinity;
      child.visible = l <= layer;
    }
  }

  // --- Slumping overlay ---

  addSlumpBead(
    x: number,
    y: number,
    z: number,
    width: number,
    height: number,
    slumpFactor: number,
    color: string = '#ff4444'
  ) {
    // Represent a slumped bead as a rounded rectangular prism
    const segs = 8;
    const w = width;
    const h = height;
    const d = width * 0.5;

    const geometry = new THREE.BoxGeometry(w, h, d);
    const c = new THREE.Color(color);
    c.lerp(new THREE.Color(0xff0000), slumpFactor);

    const material = new THREE.MeshPhysicalMaterial({
      color: c,
      metalness: 0.0,
      roughness: 0.8,
      transparent: true,
      opacity: 0.7 + slumpFactor * 0.3,
    });

    const mesh = new THREE.Mesh(geometry, material);
    mesh.position.set(x, z, y);
    mesh.userData = { slumpFactor };
    this.slumpingGroup.add(mesh);
  }

  clearSlumping() {
    while (this.slumpingGroup.children.length > 0) {
      const child = this.slumpingGroup.children[0];
      if (child instanceof THREE.Mesh) {
        child.geometry.dispose();
        (child.material as THREE.Material).dispose();
      }
      this.slumpingGroup.remove(child);
    }
  }

  // --- Camera ---

  fitCamera() {
    const box = new THREE.Box3().setFromObject(this.scene);
    const center = box.getCenter(new THREE.Vector3());
    const size = box.getSize(new THREE.Vector3());
    const maxDim = Math.max(size.x, size.y, size.z);

    if (maxDim > 0) {
      const distance = maxDim * 1.5;
      this.camera.position.set(center.x + distance * 0.7, center.y + distance * 0.7, center.z + distance * 0.7);
      this.controls.target.copy(center);
      this.controls.update();
    }
  }

  resetCamera() {
    this.camera.position.set(20, 20, 20);
    this.controls.target.set(0, 0, 0);
    this.controls.update();
  }

  // --- Render loop ---

  private startLoop() {
    const loop = () => {
      this.animationId = requestAnimationFrame(loop);
      this.controls.update();
      this.renderer.render(this.scene, this.camera);
    };
    loop();
  }

  dispose() {
    if (this.animationId !== null) {
      cancelAnimationFrame(this.animationId);
    }
    this.renderer.dispose();
    this.renderer.domElement.remove();
  }

  resize() {
    const container = this.renderer.domElement.parentElement;
    if (container) {
      const r = container.getBoundingClientRect();
      this.camera.aspect = r.width / r.height;
      this.camera.updateProjectionMatrix();
      this.renderer.setSize(r.width, r.height);
    }
  }
}
