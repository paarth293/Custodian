import { Channel, invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useCallback, useEffect, useEffectEvent, useMemo, useRef, useState } from 'react';
import './App.css';
import ApprovalView, { Plan } from './components/ApprovalView';
import ChatPanel, { Message } from './components/ChatPanel';
import CubbyCharacter from './cubby/CubbyCharacter';
import CubbyStage from './cubby/CubbyStage';
import { showCubbyMenu } from './cubby/contextMenu';
import { canBlink, resolveMood, type Activity } from './cubby/mood';
import { useBlink } from './cubby/useBlink';
import { useCubbyLife } from './cubby/useCubbyLife';
import { useDesktopPresence } from './cubby/useDesktopPresence';
import { formFor, useMode, type Mode } from './cubby/useMode';
import { call, inTauri } from './lib/tauri';

type PanelSide = 'left' | 'right';

/** How long a dragged Cubby must stay still to count as dropped. */
const LANDING_MS = 350;

function App() {
  const [panelSide, setPanelSide] = useState<PanelSide | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [isThinking, setIsThinking] = useState(false);
  const [activity, setActivity] = useState<Activity | null>(null);
  const [held, setHeld] = useState(false);
  const [bubble, setBubble] = useState<string | null>("Hi! I'm Cubby.");
  // Holds the plan awaiting user approval.
  const [pendingPlan, setPendingPlan] = useState<Plan | null>(null);

  const panelOpen = panelSide !== null;
  const life = useCubbyLife(activity !== null || held, 'happy');
  const mood = resolveMood({
    held,
    activity: pendingPlan ? 'needs-you' : activity,
    reaction: life.reaction,
    asleep: life.asleep,
  });
  const blinking = useBlink(canBlink(mood));

  const hitRef = useRef<HTMLDivElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const hitTargets = useMemo(() => [hitRef, panelRef], []);
  const presenceGaze = useDesktopPresence({
    hitTargets,
    eyes: hitRef,
    solid: panelOpen,
    slow: life.asleep,
    onNear: life.poke,
  });
  const gaze = life.look ?? presenceGaze;

  const bubbleTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const say = useCallback((text: string, ms = 3200) => {
    clearTimeout(bubbleTimer.current);
    setBubble(text);
    bubbleTimer.current = setTimeout(() => setBubble(null), ms);
  }, []);
  // Hide the start-up greeting.
  useEffect(() => {
    bubbleTimer.current = setTimeout(() => setBubble(null), 3200);
    return () => clearTimeout(bubbleTimer.current);
  }, []);

  // React when the form changes (offline ↔ online / preview).
  const mode = useMode((next: Mode, previous: Mode) => {
    if (formFor(next) === formFor(previous)) return;
    if (formFor(next) === 'special') {
      life.react('wink', 2600);
      say(next.demo && !next.online ? 'Special Form! (preview)' : "I'm online now.");
    } else {
      life.react('happy', 1600);
      say('Back to offline.');
    }
  });
  const form = formFor(mode);

  const setPanel = useCallback(async (open: boolean) => {
    if (!inTauri) {
      setPanelSide(open ? 'right' : null);
      return;
    }
    try {
      const side = await call<PanelSide>('set_panel_open', { open });
      setPanelSide(open ? (side ?? 'right') : null);
    } catch (err) {
      console.error('Could not resize the Cubby window:', err);
    }
  }, []);

  const togglePanel = useCallback(() => {
    life.poke();
    if (!panelOpen) life.react('happy', 1200);
    void setPanel(!panelOpen);
  }, [life, panelOpen, setPanel]);

  // Tray "Talk to Cubby" and Escape.
  useEffect(() => {
    if (!inTauri) return;
    const pending = listen('cubby-open-panel', () => void setPanel(true));
    return () => void pending.then((unlisten) => unlisten());
  }, [setPanel]);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && panelOpen) void setPanel(false);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [panelOpen, setPanel]);

  // Dragging: the OS moves the window; Cubby lands when moves stop.
  const landingTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const onWindowMoved = useEffectEvent(() => {
    if (!held) return;
    clearTimeout(landingTimer.current);
    landingTimer.current = setTimeout(() => {
      setHeld(false);
      life.react('happy', 1200);
    }, LANDING_MS);
  });
  useEffect(() => {
    if (!inTauri) return;
    const pending = getCurrentWindow().onMoved(() => onWindowMoved());
    return () => {
      clearTimeout(landingTimer.current);
      void pending.then((unlisten) => unlisten());
    };
  }, []);

  const onDragStart = useCallback(() => {
    if (!inTauri) return;
    life.poke();
    setHeld(true);
    landingTimer.current = setTimeout(() => setHeld(false), 1500);
    void getCurrentWindow().startDragging();
  }, [life]);

  const onDoubleClick = useCallback(() => {
    life.poke();
    life.react('excited', 2000);
    say('Yay!', 1600);
  }, [life, say]);

  const onContextMenu = useCallback(() => {
    life.poke();
    void showCubbyMenu(
      { panelOpen, asleep: life.asleep, demo: mode.demo },
      {
        togglePanel,
        toggleNap: () => {
          if (life.asleep) life.poke();
          else {
            life.sleep();
            say('Zzz…', 1800);
          }
        },
      },
    );
  }, [life, mode.demo, panelOpen, say, togglePanel]);

  const handleSendMessage = async (text: string) => {
    const userMsg: Message = { id: Date.now().toString(), role: 'user', content: text };
    setMessages((prev) => [...prev, userMsg]);
    life.poke();

    // Developer mock: typing "plan" triggers a test plan
    if (text.toLowerCase() === 'plan') {
      setPendingPlan({
        id: 'plan-12345',
        riskLevel: 'high',
        operations: [
          { path: 'C:\\Users\\Downloads\\sus_file.exe', action: 'quarantine', sizeBytes: 154000 },
          { path: 'C:\\Users\\Downloads\\malware.dll', action: 'delete', sizeBytes: 88000 },
        ],
      });
      return;
    }

    setIsThinking(true);
    setActivity('thinking');

    const replyId = (Date.now() + 1).toString();
    let started = false;
    const channel = new Channel<string>();
    channel.onmessage = (token: string) => {
      if (!started) {
        started = true;
        setIsThinking(false);
        setActivity('working');
        setMessages((prev) => [...prev, { id: replyId, role: 'assistant', content: '' }]);
      }
      setMessages((prev) =>
        prev.map((msg) => (msg.id === replyId ? { ...msg, content: msg.content + token } : msg)),
      );
    };

    try {
      await invoke('send_chat_message', { message: text, channel });
      setActivity(null);
      life.react('happy', 1600);
    } catch (err) {
      console.error('Chat error:', err);
      setActivity(null);
      life.react('curious', 2000);
      setMessages((prev) => [
        ...prev,
        { id: replyId + '-err', role: 'assistant', content: '[Connection error]' },
      ]);
    } finally {
      setIsThinking(false);
    }
  };

  const handleApprovalDecided = (approved: boolean) => {
    setMessages((prev) => [
      ...prev,
      {
        id: Date.now().toString(),
        role: 'assistant',
        content: approved ? 'Plan approved and executed successfully.' : 'Plan was rejected.',
      },
    ]);
    setPendingPlan(null);
    life.react(approved ? 'excited' : 'happy', 1800);
  };

  return (
    <div className="app" data-panel={panelSide ?? 'closed'} data-form={form}>
      {panelOpen && (
        <div ref={panelRef} className="panel">
          <header className="panel-header" data-tauri-drag-region>
            <CubbyCharacter mood={mood === 'held' ? 'idle' : mood} form={form} size={40} />
            <div className="panel-title" data-tauri-drag-region>
              <strong>Cubby</strong>
              <span className={`mode-pill ${form}`}>
                {mode.online ? 'Online' : mode.demo ? 'Offline · preview' : 'Offline'}
              </span>
            </div>
            <button
              type="button"
              className="panel-close"
              aria-label="Close chat"
              onClick={() => void setPanel(false)}
            >
              ×
            </button>
          </header>
          <ChatPanel
            messages={messages}
            isThinking={isThinking}
            onSendMessage={handleSendMessage}
          />
          {pendingPlan && <ApprovalView plan={pendingPlan} onDecided={handleApprovalDecided} />}
        </div>
      )}
      <CubbyStage
        ref={hitRef}
        mood={mood}
        form={form}
        gaze={gaze}
        blinking={blinking}
        bubble={bubble}
        demo={mode.demo && !mode.online}
        onClick={togglePanel}
        onDoubleClick={onDoubleClick}
        onDragStart={onDragStart}
        onContextMenu={onContextMenu}
      />
    </div>
  );
}

export default App;
