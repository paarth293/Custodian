import { useState } from 'react';
import CubbyCharacter from '../cubby/CubbyCharacter';
import type { Form, Mood } from '../cubby/types';

const MOODS: Mood[] = [
  'idle',
  'happy',
  'curious',
  'thinking',
  'working',
  'excited',
  'sleeping',
  'wink',
  'held',
];

/**
 * Development-only preview of every mood in both forms. Open the dev server
 * with `?gallery` (`npm run dev -w apps/desktop`, then
 * http://localhost:1420/?gallery). Not included in production builds.
 */
export default function Gallery() {
  const [blinking, setBlinking] = useState(false);
  const forms: Form[] = ['home', 'special'];

  return (
    <main className="gallery">
      <header>
        <h1>Cubby gallery</h1>
        <button type="button" onClick={() => setBlinking((b) => !b)}>
          {blinking ? 'Open eyes' : 'Blink'}
        </button>
      </header>
      {forms.map((form) => (
        <section key={form} className={`gallery-row ${form}`}>
          <h2>{form === 'home' ? 'Original Form (offline)' : 'Special Form (online)'}</h2>
          <div className="gallery-hero">
            <CubbyCharacter
              mood={form === 'home' ? 'idle' : 'wink'}
              form={form}
              size={320}
              blinking={blinking}
            />
          </div>
          <div className="gallery-grid">
            {MOODS.map((mood) => (
              <figure key={mood}>
                <CubbyCharacter mood={mood} form={form} size={150} blinking={blinking} />
                <figcaption>{mood}</figcaption>
              </figure>
            ))}
          </div>
        </section>
      ))}
    </main>
  );
}
