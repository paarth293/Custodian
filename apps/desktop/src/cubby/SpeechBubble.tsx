interface SpeechBubbleProps {
  text: string | null;
}

/** A soft glass speech bubble above Cubby's head. */
export default function SpeechBubble({ text }: SpeechBubbleProps) {
  return (
    <div className={`speech-bubble${text ? ' visible' : ''}`} role="status" aria-live="polite">
      {text}
    </div>
  );
}
