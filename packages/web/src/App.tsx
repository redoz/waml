import { CanvasApp } from "./components/canvas/Canvas";

// Local-only app: the model lives in localStorage (+ URL share). No accounts,
// no auth, no cloud — anonymous load just shows the canvas.
export function App() {
  return <CanvasApp />;
}
