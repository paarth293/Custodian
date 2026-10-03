import { useRive } from '@rive-app/react-canvas';
import { useState } from 'react';

/** Animation asset; place the exported Rive file at public/cubby.riv. */
const CUBBY_ASSET = '/cubby.riv';

export default function Cubby() {
  const [failed, setFailed] = useState(false);
  const { RiveComponent } = useRive({
    src: CUBBY_ASSET,
    stateMachines: 'State Machine 1',
    autoplay: true,
    onLoadError: () => setFailed(true),
  });

  return (
    <div className="cubby">
      {failed ? <div className="cubby-fallback" aria-hidden /> : <RiveComponent />}
    </div>
  );
}
