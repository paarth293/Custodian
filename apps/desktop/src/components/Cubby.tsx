import riveWasmUrl from '@rive-app/canvas/rive.wasm?url';
import { RuntimeLoader, useRive, useStateMachineInput } from '@rive-app/react-canvas';
import { useState, useEffect } from 'react';
import { useCubbyStore } from '../state/cubby';
import { invoke } from '@tauri-apps/api/core';

RuntimeLoader.setWasmUrl(riveWasmUrl);

const CUBBY_ASSET = '/cubby.riv';
const STATE_MACHINE_NAME = 'Cubby';

export default function Cubby() {
  const [failed, setFailed] = useState(false);
  const { RiveComponent, rive } = useRive({
    src: CUBBY_ASSET,
    stateMachines: STATE_MACHINE_NAME,
    autoplay: true,
    onLoadError: () => setFailed(true),
  });

  const stateInput = useStateMachineInput(rive, STATE_MACHINE_NAME, 'state');
  const onlineInput = useStateMachineInput(rive, STATE_MACHINE_NAME, 'online');
  const badgeInput = useStateMachineInput(rive, STATE_MACHINE_NAME, 'badge');
  const reducedMotionInput = useStateMachineInput(rive, STATE_MACHINE_NAME, 'reduced_motion');

  const { state, online, badge, reduced_motion } = useCubbyStore();

  useEffect(() => {
    if (stateInput) stateInput.value = state;
  }, [state, stateInput]);

  useEffect(() => {
    if (onlineInput) onlineInput.value = online;
  }, [online, onlineInput]);

  useEffect(() => {
    if (badgeInput) badgeInput.value = badge;
  }, [badge, badgeInput]);

  useEffect(() => {
    if (reducedMotionInput) reducedMotionInput.value = reduced_motion;
  }, [reduced_motion, reducedMotionInput]);

  // Toggle click-through: ignore cursor when NOT hovering the Cubby
  return (
    <div 
      className="cubby"
      onPointerEnter={() => invoke('set_ignore_cursor_events', { ignore: false })}
      onPointerLeave={() => invoke('set_ignore_cursor_events', { ignore: true })}
    >
      {failed ? <div className="cubby-fallback" aria-hidden /> : <RiveComponent />}
      
      {/* Dev-only controls for testing the Rive state machine bindings */}
      <div className="cubby-dev-controls" style={{ position: 'absolute', bottom: -50, fontSize: '10px' }}>
        <button onClick={() => useCubbyStore.getState().setState((state + 1) % 12)}>State: {state}</button>
        <button onClick={() => useCubbyStore.getState().setOnline(!online)}>{online ? 'Online' : 'Offline'}</button>
      </div>
    </div>
  );
}
