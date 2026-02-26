interface CommandOutputProps {
  output: string;
  onDismiss: () => void;
}

export function CommandOutput({ output, onDismiss }: CommandOutputProps) {
  return (
    <div className="command-output">
      <div className="command-output-header">
        <span className="command-output-title">Output</span>
        <button className="command-output-close" onClick={onDismiss}>
          &#x2715;
        </button>
      </div>
      <pre className="command-output-body">{output}</pre>
    </div>
  );
}
