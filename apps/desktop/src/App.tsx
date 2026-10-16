import { Channel, invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useCallback, useEffect, useEffectEvent, useMemo, useRef, useState } from 'react';
import './App.css';
import ApprovalView from './components/ApprovalView';
import ChatPanel, { Message } from './components/ChatPanel';
import PrivacyControls from './components/PrivacyControls';
import { appendReply, bubbleText, decideCard, type ChatEvent } from './chat/events';
import { updateWork, type ConversationWork } from './chat/work';
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
  const [work, setWork] = useState<ConversationWork>({ turns: {}, plans: [] });
  const [isResetting, setIsResetting] = useState(false);
  const resetInProgress = useRef(false);
  const conversationGeneration = useRef(0);
  const isThinking = Object.keys(work.turns).length > 0;
  const status =
    Object.values(work.turns)
      .filter((text) => text !== null)
      .at(-1) ?? null;
  const activity: Activity | null = isThinking ? (status ? 'working' : 'thinking') : null;
  const [held, setHeld] = useState(false);
  const [bubble, setBubble] = useState<string | null>("Hi! I'm Cubby.");

  const panelOpen = panelSide !== null;
  const life = useCubbyLife(activity !== null || held, 'happy');
  const mood = resolveMood({
    held,
    activity: work.plans.length > 0 ? 'needs-you' : activity,
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
  // Something Cubby noticed on its own (e.g. space freed after 30 days):
  // a speech bubble now, and a line in the chat for later.
  const onNotice = useEffectEvent((text: string) => {
    life.poke();
    life.react('happy', 1600);
    say(bubbleText(text), 9000);
    setMessages((prev) => [
      ...prev,
      { id: `${Date.now()}-notice`, role: 'assistant', content: text },
    ]);
  });
  useEffect(() => {
    if (!inTauri) return;
    const pending = listen<string>('cubby-notice', (event) => onNotice(event.payload));
    return () => void pending.then((unlisten) => unlisten());
  }, []);
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
    if (resetInProgress.current) return;
    const generation = conversationGeneration.current;
    const turnId = crypto.randomUUID();
    const userMsg: Message = { id: turnId, role: 'user', content: text };
    setMessages((prev) => [...prev, userMsg]);
    life.poke();
    setWork((prev) => updateWork(prev, { kind: 'start', id: turnId }));

    // Text streams into the current reply bubble. A tool result or a card
    // stands on its own, and text after it starts a new bubble, so the AI's
    // words never mix with what Cubby's code found.
    const base = `${turnId}-reply`;
    let part = 0;
    let replyId = base;
    const addBlock = (block: Message) => {
      setMessages((prev) => [...prev, block]);
      replyId = `${base}-${++part}`;
    };
    const channel = new Channel<ChatEvent>();
    channel.onmessage = (event) => {
      if (generation !== conversationGeneration.current) return;
      switch (event.kind) {
        case 'status':
          setWork((prev) => updateWork(prev, { kind: 'status', id: turnId, text: event.text }));
          break;
        case 'text': {
          const id = replyId;
          setMessages((prev) => appendReply(prev, id, event.text));
          break;
        }
        case 'result':
          addBlock({
            id: `${base}-r${part}`,
            role: 'assistant',
            content: event.text,
            result: true,
          });
          break;
        case 'confirm':
          addBlock({ id: event.card.id, role: 'assistant', content: '', card: event.card });
          break;
        case 'plan':
          setWork((prev) => updateWork(prev, { kind: 'plan', id: turnId, plan: event.plan }));
          break;
        case 'action':
          if (event.action === 'sleep') setTimeout(() => life.sleep(), 1500);
          break;
      }
    };

    try {
      await invoke('send_chat_message', { message: text, channel });
    } catch (err) {
      console.error('Chat error:', err);
      life.react('curious', 2000);
      const id = replyId;
      setMessages((prev) => appendReply(prev, id, `Sorry, something went wrong: ${String(err)}`));
    } finally {
      setWork((prev) => updateWork(prev, { kind: 'finish', id: turnId }));
    }
  };

  const handleNewChat = async () => {
    if (isThinking || resetInProgress.current) return;
    resetInProgress.current = true;
    setIsResetting(true);
    try {
      await call('reset_conversation');
    } catch (err) {
      console.error('Failed to reset conversation:', err);
      setMessages((prev) => [
        ...prev,
        {
          id: `${Date.now()}-reset-error`,
          role: 'assistant',
          content: `I couldn't start a new conversation: ${String(err)}`,
        },
      ]);
      return;
    } finally {
      resetInProgress.current = false;
      setIsResetting(false);
    }
    conversationGeneration.current++;
    setMessages([]);
    setWork((prev) => updateWork(prev, { kind: 'reset' }));
    say('Started a new conversation.');
  };

  const handleStop = () => {
    void call('cancel_chat').catch(() => undefined);
  };

  /** Sends the person's answer to a card; Rust runs what it stored for it. */
  const handleDecideCard = async (messageId: string, allow: boolean) => {
    const generation = conversationGeneration.current;
    setMessages((prev) => decideCard(prev, messageId, allow ? 'allowed' : 'declined'));
    let answer: string;
    let failed = false;
    try {
      answer = (await call<string>('decide_action', { actionId: messageId, allow })) ?? '';
    } catch (err) {
      answer = String(err);
      failed = true;
    }
    if (generation !== conversationGeneration.current) return;
    if (answer) {
      setMessages((prev) => [
        ...prev,
        { id: `${messageId}-answer`, role: 'assistant', content: answer },
      ]);
    }
    if (
      failed ||
      answer.toLowerCase().startsWith('failed') ||
      answer.toLowerCase().startsWith('error')
    ) {
      life.react('curious', 2000);
    }
  };

  const uiGeneration = conversationGeneration.current;
  const handleApprovalDecided = (planId: string, _approved: boolean, result: string) => {
    if (uiGeneration !== conversationGeneration.current) return;
    setMessages((prev) => [
      ...prev,
      { id: `${Date.now()}-result`, role: 'assistant', content: result },
    ]);
    setWork((prev) => updateWork(prev, { kind: 'decided', planId }));
    const failed =
      result.toLowerCase().startsWith('failed') || result.toLowerCase().startsWith('error');
    if (failed) {
      life.react('curious', 2000);
    }
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
              className="panel-new-chat"
              aria-label="New conversation"
              title="New conversation"
              disabled={isThinking || isResetting}
              onClick={() => void handleNewChat()}
            >
              +
            </button>
            <button
              type="button"
              className="panel-close"
              aria-label="Close chat"
              onClick={() => void setPanel(false)}
            >
              ×
            </button>
          </header>
          <PrivacyControls
            onChanged={() => {
              conversationGeneration.current++;
              setMessages([]);
              setWork((previous) => updateWork(previous, { kind: 'reset' }));
            }}
          />
          <ChatPanel
            messages={messages}
            isThinking={isThinking}
            status={status}
            onSendMessage={handleSendMessage}
            disabled={isResetting}
            onStop={handleStop}
            onDecideCard={(id, allow) => void handleDecideCard(id, allow)}
          />
          {work.plans.map((plan) => (
            <ApprovalView
              key={plan.id}
              plan={plan}
              onDecided={(approved, result) => handleApprovalDecided(plan.id, approved, result)}
            />
          ))}
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
