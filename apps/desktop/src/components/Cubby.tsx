import { useRive } from '@rive-app/react-canvas';

export default function Cubby() {
  const { RiveComponent } = useRive({
    src: '/cubby.riv',
    stateMachines: 'State Machine 1',
    autoplay: true,
  });

  return (
    <div style={{ width: '200px', height: '200px', backgroundColor: '#333' }}>
      <RiveComponent />
    </div>
  );
}
