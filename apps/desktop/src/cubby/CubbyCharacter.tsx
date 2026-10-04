import { useId, useMemo } from 'react';
import { fluffPath } from './fluff';
import type { Form, Gaze, Mood } from './types';
import './cubby.css';

export interface CubbyCharacterProps {
  mood: Mood;
  form: Form;
  /** Where the eyes look; -1..1 on each axis. */
  gaze?: Gaze;
  /** True for the brief moment the eyes are closed in a blink. */
  blinking?: boolean;
  /** Rendered size in CSS pixels (the art is drawn on a 240×240 grid). */
  size?: number;
  className?: string;
}

const HEAD = fluffPath({
  cx: 120,
  cy: 117,
  rx: 66,
  ry: 55,
  tufts: 20,
  depth: 0.02,
  seed: 5,
  cheeks: 0.15,
});
const BODY = fluffPath({ cx: 120, cy: 197, rx: 37, ry: 27, tufts: 11, depth: 0.04, seed: 11 });
const TAIL_BASE = fluffPath({ cx: 152, cy: 207, rx: 14, ry: 10, tufts: 14, depth: 0.05, seed: 3 });

// Broad, petal-like crystal ears with rounded tips (right ear is mirrored).
const EAR_L = 'M76 108 C50 104 20 80 22 40 C27 33 37 33 46 39 C67 53 86 66 96 86 Z';
const EAR_L_INNER = 'M78 100 C58 94 36 76 32 50 C36 46 42 46 48 50 C64 62 78 72 86 88 Z';

const EYES_OPEN: ReadonlySet<Mood> = new Set(['idle', 'curious', 'thinking', 'working', 'held']);

/**
 * Cubby, drawn as layered vector art. Colours come from CSS custom
 * properties on `.cubby[data-form]`, so switching form morphs the palette
 * smoothly. Animation is CSS-driven (see `cubby.css`), keyed off
 * `data-mood`, so the component never re-renders per frame.
 */
export default function CubbyCharacter({
  mood,
  form,
  gaze = { x: 0, y: 0 },
  blinking = false,
  size = 200,
  className,
}: CubbyCharacterProps) {
  const raw = useId();
  const uid = useMemo(() => `c${raw.replace(/[^a-zA-Z0-9]/g, '')}`, [raw]);
  const ref = (name: string) => `url(#${uid}-${name})`;
  const id = (name: string) => `${uid}-${name}`;

  // Pupils move a little toward the gaze; the iris highlight stays put so the
  // eyes keep their glossy look.
  const look =
    mood === 'thinking' ? { x: -0.6, y: -0.8 } : mood === 'working' ? { x: 0, y: 0.7 } : gaze;
  const px = Math.max(-1, Math.min(1, look.x)) * 4.5;
  const py = Math.max(-1, Math.min(1, look.y)) * 3.5;

  const eyesOpen = EYES_OPEN.has(mood);
  const leftEye =
    mood === 'wink' ? 'happy' : eyesOpen ? 'open' : mood === 'sleeping' ? 'closed' : 'happy';
  const rightEye = mood === 'wink' ? 'open' : leftEye;

  return (
    <svg
      className={`cubby${className ? ` ${className}` : ''}`}
      data-form={form}
      data-mood={mood}
      data-blinking={blinking ? 'true' : 'false'}
      width={size}
      height={size}
      viewBox="0 0 240 240"
      role="img"
      aria-label={`Cubby is ${mood === 'held' ? 'being carried' : mood}`}
    >
      <defs>
        <radialGradient id={id('fur')} cx="42%" cy="34%" r="72%">
          <stop offset="0%" style={{ stopColor: 'var(--fur-1)' }} />
          <stop offset="62%" style={{ stopColor: 'var(--fur-2)' }} />
          <stop offset="100%" style={{ stopColor: 'var(--fur-3)' }} />
        </radialGradient>
        <radialGradient id={id('body')} cx="45%" cy="30%" r="80%">
          <stop offset="0%" style={{ stopColor: 'var(--fur-1)' }} />
          <stop offset="70%" style={{ stopColor: 'var(--fur-2)' }} />
          <stop offset="100%" style={{ stopColor: 'var(--fur-3)' }} />
        </radialGradient>
        <linearGradient id={id('crystal')} x1="0" y1="1" x2="1" y2="0">
          <stop offset="0%" style={{ stopColor: 'var(--crystal-3)' }} />
          <stop offset="55%" style={{ stopColor: 'var(--crystal-2)' }} />
          <stop offset="100%" style={{ stopColor: 'var(--crystal-1)' }} />
        </linearGradient>
        <linearGradient id={id('crystal-r')} x1="1" y1="1" x2="0" y2="0">
          <stop offset="0%" style={{ stopColor: 'var(--crystal-3)' }} />
          <stop offset="55%" style={{ stopColor: 'var(--crystal-2)' }} />
          <stop offset="100%" style={{ stopColor: 'var(--crystal-1)' }} />
        </linearGradient>
        <radialGradient id={id('iris')} cx="50%" cy="70%" r="70%">
          <stop offset="0%" style={{ stopColor: 'var(--iris-1)' }} />
          <stop offset="55%" style={{ stopColor: 'var(--iris-2)' }} />
          <stop offset="100%" style={{ stopColor: 'var(--iris-3)' }} />
        </radialGradient>
        <linearGradient id={id('scarf')} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" style={{ stopColor: 'var(--scarf-1)' }} />
          <stop offset="100%" style={{ stopColor: 'var(--scarf-2)' }} />
        </linearGradient>
        <linearGradient id={id('gem')} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0%" style={{ stopColor: 'var(--gem-1)' }} />
          <stop offset="100%" style={{ stopColor: 'var(--gem-2)' }} />
        </linearGradient>
        <radialGradient id={id('orb')} cx="38%" cy="35%" r="65%">
          <stop offset="0%" style={{ stopColor: 'var(--orb-1)' }} />
          <stop offset="45%" style={{ stopColor: 'var(--orb-2)' }} />
          <stop offset="100%" style={{ stopColor: 'var(--orb-3)' }} />
        </radialGradient>
        <radialGradient id={id('screen')} cx="50%" cy="40%" r="70%">
          <stop offset="0%" style={{ stopColor: 'var(--crystal-1)' }} />
          <stop offset="100%" style={{ stopColor: 'var(--crystal-2)' }} />
        </radialGradient>
        <filter id={id('glow')} x="-50%" y="-50%" width="200%" height="200%">
          <feGaussianBlur stdDeviation="5" />
        </filter>
        <filter id={id('soft')} x="-50%" y="-50%" width="200%" height="200%">
          <feGaussianBlur stdDeviation="2.4" />
        </filter>
      </defs>

      {/* Ground shadow */}
      <ellipse className="cubby-shadow" cx="120" cy="229" rx="50" ry="6" filter={ref('soft')} />

      <g className="cubby-figure">
        {/* Tail: a glowing crystal leaf behind the body */}
        <g className="cubby-tail">
          <path
            d="M148 206 C172 214 206 198 214 166 C219 146 214 126 205 114 C196 136 178 150 164 166 C153 180 148 194 148 206 Z"
            fill="var(--crystal-glow)"
            filter={ref('glow')}
            opacity="0.55"
          />
          <path
            d="M148 206 C172 214 206 198 214 166 C219 146 214 126 205 114 C196 136 178 150 164 166 C153 180 148 194 148 206 Z"
            fill={ref('crystal-r')}
            opacity="0.92"
          />
          <path d="M152 202 C172 190 192 164 205 124" className="cubby-vein" />
          <path d="M171 190 C178 186 186 184 196 186" className="cubby-vein thin" />
          <path d="M184 172 C190 168 198 166 207 168" className="cubby-vein thin" />
          <path d="M190 182 L196 174 L202 182 L196 190 Z" fill={ref('gem')} opacity="0.9" />
          <path d={TAIL_BASE} fill={ref('body')} />
        </g>

        {/* Body */}
        <g className="cubby-body">
          <path d={BODY} fill={ref('body')} />
          <ellipse cx="120" cy="203" rx="20" ry="13" fill="var(--fur-1)" opacity="0.7" />
          <g className="cubby-foot left">
            <ellipse cx="104" cy="221" rx="14" ry="8.5" fill={ref('body')} />
            <ellipse cx="102" cy="225" rx="9" ry="4" fill="var(--paw)" opacity="0.85" />
          </g>
          <g className="cubby-foot right">
            <ellipse cx="136" cy="221" rx="14" ry="8.5" fill={ref('body')} />
            <ellipse cx="138" cy="225" rx="9" ry="4" fill="var(--paw)" opacity="0.85" />
          </g>
          <g className="cubby-arm left">
            <ellipse
              cx="94"
              cy="202"
              rx="8"
              ry="11"
              transform="rotate(16 94 202)"
              fill={ref('body')}
            />
            <ellipse cx="92" cy="211" rx="5" ry="3" fill="var(--paw)" opacity="0.7" />
          </g>
          <g className="cubby-arm right">
            <ellipse
              cx="146"
              cy="202"
              rx="8"
              ry="11"
              transform="rotate(-16 146 202)"
              fill={ref('body')}
            />
            <ellipse cx="148" cy="211" rx="5" ry="3" fill="var(--paw)" opacity="0.7" />
          </g>
        </g>

        {/* Scarf with the gem */}
        <g className="cubby-scarf">
          <path
            d="M84 166 C102 177 138 177 156 166 C157 171 156 175 154 178 C138 187 102 187 86 178 C84 175 83 171 84 166 Z"
            fill={ref('scarf')}
          />
          <path d="M90 171 C106 180 134 180 150 171" className="cubby-scarf-fold" />
          <path
            d="M113 184 C108 191 104 197 101 202 C107 201 112 198 116 194 Z"
            fill={ref('scarf')}
          />
          <path
            d="M127 184 C132 191 136 197 139 202 C133 201 128 198 124 194 Z"
            fill={ref('scarf')}
          />
          <ellipse cx="120" cy="183" rx="9" ry="7.5" fill={ref('scarf')} />
          <path
            d="M120 172 L128 183 L120 195 L112 183 Z"
            fill="var(--crystal-glow)"
            filter={ref('soft')}
            opacity="0.85"
          />
          <path d="M120 173 L127 183 L120 194 L113 183 Z" fill={ref('gem')} />
          <path d="M120 173 L122.5 183 L120 194 M113 183 L127 183" className="cubby-facet" />
        </g>

        {/* Head, ears and antenna tilt together */}
        <g className="cubby-head">
          <g className="cubby-ear left">
            <path d={EAR_L} fill="var(--crystal-glow)" filter={ref('glow')} opacity="0.5" />
            <path d={EAR_L} fill={ref('crystal')} opacity="0.94" />
            <path d={EAR_L_INNER} fill="var(--crystal-1)" opacity="0.4" />
            <path d="M84 96 C66 84 46 66 34 46" className="cubby-vein" />
          </g>
          <g className="cubby-ear right">
            <path
              d={EAR_L}
              transform="matrix(-1 0 0 1 240 0)"
              fill="var(--crystal-glow)"
              filter={ref('glow')}
              opacity="0.5"
            />
            <path
              d={EAR_L}
              transform="matrix(-1 0 0 1 240 0)"
              fill={ref('crystal-r')}
              opacity="0.94"
            />
            <path
              d={EAR_L_INNER}
              transform="matrix(-1 0 0 1 240 0)"
              fill="var(--crystal-1)"
              opacity="0.4"
            />
            <path d="M156 96 C174 84 194 66 206 46" className="cubby-vein" />
          </g>
          <path
            d="M62 134 C42 140 24 134 16 118 C26 112 36 110 46 113 C54 116 60 122 66 126 Z"
            fill={ref('crystal')}
            opacity="0.85"
          />
          <path
            d="M178 134 C198 140 216 134 224 118 C214 112 204 110 194 113 C186 116 180 122 174 126 Z"
            fill={ref('crystal-r')}
            opacity="0.85"
          />

          {/* Cheek tufts */}
          <path d="M60 138 C52 146 50 154 54 160 C60 154 66 150 72 148 Z" fill={ref('fur')} />
          <path
            d="M180 138 C188 146 190 154 186 160 C180 154 174 150 168 148 Z"
            fill={ref('fur')}
          />
          <path className="cubby-head-fur" d={HEAD} fill={ref('fur')} />
          <ellipse
            cx="120"
            cy="150"
            rx="52"
            ry="18"
            fill="var(--fur-3)"
            opacity="0.35"
            filter={ref('glow')}
          />
          <ellipse
            cx="102"
            cy="84"
            rx="26"
            ry="14"
            fill="#fff"
            opacity="0.7"
            filter={ref('soft')}
          />

          {/* Head tuft */}
          <path
            d="M108 70 C100 58 101 48 108 42 C114 52 116 62 114 70 Z"
            fill={ref('crystal')}
            opacity="0.85"
          />
          <path
            d="M132 70 C140 58 139 48 132 42 C126 52 124 62 126 70 Z"
            fill={ref('crystal-r')}
            opacity="0.85"
          />
          <path d="M120 68 C112 56 114 42 120 34 C126 42 128 56 120 68 Z" fill={ref('crystal')} />

          {/* Antenna with its glowing orb */}
          <g className="cubby-antenna">
            <path d="M120 40 C117 28 112 18 118 10 C123 3 135 4 138 12" className="cubby-stalk" />
            <circle
              className="cubby-orb-halo"
              cx="138"
              cy="18"
              r="14"
              fill="var(--orb-2)"
              filter={ref('glow')}
            />
            <circle cx="138" cy="18" r="8.5" fill={ref('orb')} />
            <circle cx="135.2" cy="15.2" r="2.6" fill="#fff" opacity="0.9" />
          </g>

          {/* Blush */}
          <ellipse className="cubby-blush" cx="72" cy="152" rx="11" ry="6" filter={ref('soft')} />
          <ellipse className="cubby-blush" cx="168" cy="152" rx="11" ry="6" filter={ref('soft')} />

          <Eye cx={93} kind={leftEye} iris={ref('iris')} px={px} py={py} />
          <Eye cx={147} kind={rightEye} iris={ref('iris')} px={px} py={py} />

          <g transform="translate(0 6)">
            <Mouth mood={mood} />
          </g>
        </g>

        {/* Laptop while working */}
        <g className="cubby-laptop">
          <path d="M86 222 L154 222 L162 230 L78 230 Z" fill="#cfd8e8" />
          <path d="M92 190 L148 190 L152 222 L88 222 Z" fill="#e9eef7" />
          <path d="M96 194 L144 194 L147 218 L93 218 Z" fill={ref('screen')} />
          <path d="M120 200 L127 206 L120 214 L113 206 Z" fill="#fff" opacity="0.85" />
        </g>
      </g>

      {/* Mood overlays */}
      <text className="cubby-question" x="196" y="64">
        ?
      </text>
      <g className="cubby-thinking-dots">
        <circle cx="186" cy="66" r="4" />
        <circle cx="199" cy="60" r="4" />
        <circle cx="212" cy="54" r="4" />
      </g>
      <g className="cubby-zzz">
        <text x="178" y="78">
          z
        </text>
        <text x="194" y="60">
          z
        </text>
        <text x="212" y="40">
          Z
        </text>
      </g>
      <g className="cubby-sparkles">
        <path d="M30 70 l3 8 8 3 -8 3 -3 8 -3 -8 -8 -3 8 -3 Z" />
        <path d="M206 92 l2.5 6 6 2.5 -6 2.5 -2.5 6 -2.5 -6 -6 -2.5 6 -2.5 Z" />
        <path d="M40 150 l2 5 5 2 -5 2 -2 5 -2 -5 -5 -2 5 -2 Z" />
        <path d="M196 44 l2 5 5 2 -5 2 -2 5 -2 -5 -5 -2 5 -2 Z" />
      </g>
      <path
        className="cubby-heart"
        d="M196 104 c-4 -6 -14 -4 -12 4 c1 5 12 11 12 11 s11 -6 12 -11 c2 -8 -8 -10 -12 -4 Z"
      />
    </svg>
  );
}

interface EyeProps {
  cx: number;
  kind: 'open' | 'happy' | 'closed';
  iris: string;
  px: number;
  py: number;
}

function Eye({ cx, kind, iris, px, py }: EyeProps) {
  const cy = 128;
  if (kind === 'happy') {
    return (
      <path
        className="cubby-eye-arc"
        d={`M${cx - 14} ${cy + 4} Q${cx} ${cy - 13} ${cx + 14} ${cy + 4}`}
      />
    );
  }
  if (kind === 'closed') {
    return (
      <path className="cubby-eye-arc" d={`M${cx - 13} ${cy} Q${cx} ${cy + 10} ${cx + 13} ${cy}`} />
    );
  }
  return (
    <g className="cubby-eye" style={{ transformOrigin: `${cx}px ${cy}px` }}>
      <ellipse cx={cx} cy={cy} rx="19" ry="22" fill="var(--eye-dark)" />
      <g style={{ transform: `translate(${px}px, ${py}px)` }} className="cubby-pupil">
        <ellipse cx={cx} cy={cy + 1.5} rx="15.5" ry="18.5" fill={iris} />
        <ellipse cx={cx} cy={cy + 3} rx="7.5" ry="9.5" fill="var(--eye-dark)" opacity="0.8" />
      </g>
      <ellipse cx={cx + 6} cy={cy - 9} rx="7" ry="8" fill="#fff" />
      <circle cx={cx - 7} cy={cy + 10} r="3" fill="#fff" opacity="0.9" />
      <circle cx={cx + 9} cy={cy + 7} r="1.6" fill="#fff" opacity="0.8" />
    </g>
  );
}

function Mouth({ mood }: { mood: Mood }) {
  switch (mood) {
    case 'curious':
    case 'held':
      return <ellipse className="cubby-mouth" cx="120" cy="151" rx="4" ry="5" />;
    case 'sleeping':
      return <path className="cubby-mouth-line" d="M115 150 Q120 153 125 150" />;
    case 'thinking':
    case 'working':
      return <path className="cubby-mouth-line" d="M113 149 Q120 154 127 149" />;
    case 'excited':
    case 'happy':
    case 'wink':
      return (
        <g>
          <path
            className="cubby-mouth"
            d="M107 144 C109 160 131 160 133 144 C125 148 115 148 107 144 Z"
          />
          <path
            className="cubby-tongue"
            d="M113 152 C115 158 125 158 127 152 C123 150 117 150 113 152 Z"
          />
        </g>
      );
    default:
      return (
        <g>
          <path
            className="cubby-mouth"
            d="M110 145 C112 156 128 156 130 145 C124 148 116 148 110 145 Z"
          />
          <path
            className="cubby-tongue"
            d="M114.5 151 C116.5 155 123.5 155 125.5 151 C122 150 118 150 114.5 151 Z"
          />
        </g>
      );
  }
}
