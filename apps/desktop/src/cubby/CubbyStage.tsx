import { forwardRef, useRef, type PointerEvent } from 'react';
import CubbyCharacter from './CubbyCharacter';
import SpeechBubble from './SpeechBubble';
import type { Form, Gaze, Mood } from './types';

/** Movement (px) before a press becomes a drag instead of a click. */
const DRAG_THRESHOLD = 4;
/** Window for a second click to count as a double-click. */
const DOUBLE_CLICK_MS = 240;

export interface CubbyStageProps {
  mood: Mood;
  form: Form;
  gaze: Gaze;
  blinking: boolean;
  bubble: string | null;
  demo: boolean;
  onClick: () => void;
  onDoubleClick: () => void;
  onDragStart: () => void;
  onContextMenu: () => void;
}

/**
 * Cubby plus its speech bubble. The forwarded ref is the hit area used for
 * click-through; pressing and moving drags the whole window.
 */
const CubbyStage = forwardRef<HTMLDivElement, CubbyStageProps>(function CubbyStage(
  { mood, form, gaze, blinking, bubble, demo, onClick, onDoubleClick, onDragStart, onContextMenu },
  hitRef,
) {
  const press = useRef<{ x: number; y: number; dragging: boolean } | null>(null);
  const clickTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  const activate = () => {
    clearTimeout(clickTimer.current);
    clickTimer.current = undefined;
    press.current = null;
    onClick();
  };

  const onPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    press.current = { x: e.clientX, y: e.clientY, dragging: false };
  };

  const onPointerMove = (e: PointerEvent<HTMLDivElement>) => {
    const p = press.current;
    if (!p || p.dragging) return;
    if (Math.hypot(e.clientX - p.x, e.clientY - p.y) > DRAG_THRESHOLD) {
      p.dragging = true;
      onDragStart();
    }
  };

  const onPointerUp = () => {
    const p = press.current;
    press.current = null;
    if (!p || p.dragging) return;
    if (clickTimer.current) {
      clearTimeout(clickTimer.current);
      clickTimer.current = undefined;
      onDoubleClick();
      return;
    }
    clickTimer.current = setTimeout(() => {
      clickTimer.current = undefined;
      onClick();
    }, DOUBLE_CLICK_MS);
  };

  return (
    <div className="cubby-stage">
      <SpeechBubble text={bubble} />
      <div
        ref={hitRef}
        className="cubby-hit"
        role="button"
        tabIndex={0}
        aria-label="Cubby. Click to talk, drag to move, right-click for options."
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={() => (press.current = null)}
        onClick={(event) => {
          // Accessibility/default-action clicks have no pointer click count.
          // Physical pointer clicks already use the gesture timer above.
          if (event.detail === 0) activate();
        }}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            activate();
          }
        }}
        onContextMenu={(e) => {
          e.preventDefault();
          onContextMenu();
        }}
      >
        <CubbyCharacter mood={mood} form={form} gaze={gaze} blinking={blinking} size={196} />
        {demo && <span className="cubby-demo-chip">demo</span>}
      </div>
    </div>
  );
});

export default CubbyStage;
