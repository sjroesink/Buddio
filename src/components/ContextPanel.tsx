import type { PastedImage } from "./SearchBar";

interface ContextInfo {
  hasSelection: boolean;
  hasClipboard: boolean;
  sourceApp: string | null;
  sourceProcessName: string | null;
  selectedText?: string | null;
  clipboardText?: string | null;
}

interface ContextPanelProps {
  contextInfo: ContextInfo;
  images: PastedImage[];
  onRemoveSelection?: () => void;
  onRemoveClipboard?: () => void;
  onRemoveImage?: (id: string) => void;
}

function RemoveButton({ onClick }: { onClick: () => void }) {
  return (
    <button
      onClick={(e) => {
        e.stopPropagation();
        onClick();
      }}
      className="p-0.5 rounded text-launcher-muted/40 hover:text-red-400 hover:bg-red-500/10 transition-colors"
      title="Remove"
    >
      <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
        <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
      </svg>
    </button>
  );
}

function truncate(text: string, max: number) {
  return text.length > max ? text.slice(0, max) + "\u2026" : text;
}

export function ContextPanel({
  contextInfo,
  images,
  onRemoveSelection,
  onRemoveClipboard,
  onRemoveImage,
}: ContextPanelProps) {
  const hasSourceApp = !!contextInfo.sourceApp;
  const hasAny =
    hasSourceApp || contextInfo.hasSelection || contextInfo.hasClipboard || images.length > 0;

  if (!hasAny) return null;

  // Derive a short app name from the process name (e.g. "Code.exe" → "Code")
  const appName = contextInfo.sourceProcessName
    ? contextInfo.sourceProcessName.replace(/\.exe$/i, "")
    : null;

  return (
    <div className="flex-1 flex flex-col min-h-0 overflow-y-auto">
      {/* Header */}
      <div className="px-4 py-2 border-b border-launcher-border/30 flex items-center justify-between shrink-0">
        <span className="text-[11px] text-launcher-muted uppercase tracking-wider font-medium">
          Context
        </span>
      </div>

      {/* Items */}
      <div className="flex-1 overflow-y-auto">
        {/* Source application */}
        {hasSourceApp && (
          <div className="px-4 py-2.5 border-b border-launcher-border/20 hover:bg-launcher-hover/30 transition-colors">
            <div className="flex items-center gap-1.5 mb-1">
              <span className="inline-flex items-center px-1.5 py-0.5 rounded text-[9px] font-medium bg-orange-500/20 text-orange-300 border border-orange-500/30">
                APP
              </span>
              <span className="text-[10px] text-launcher-muted">
                {appName ? `Source — ${appName}` : "Source application"}
              </span>
            </div>
            <div className="text-xs text-launcher-text/80 truncate leading-relaxed">
              {contextInfo.sourceApp}
            </div>
          </div>
        )}

        {/* Selected text */}
        {contextInfo.hasSelection && contextInfo.selectedText && (
          <div className="px-4 py-2.5 border-b border-launcher-border/20 hover:bg-launcher-hover/30 transition-colors">
            <div className="flex items-center justify-between mb-1">
              <div className="flex items-center gap-1.5">
                <span className="inline-flex items-center px-1.5 py-0.5 rounded text-[9px] font-medium bg-purple-500/20 text-purple-300 border border-purple-500/30">
                  SEL
                </span>
                <span className="text-[10px] text-launcher-muted">Selected text</span>
              </div>
              {onRemoveSelection && <RemoveButton onClick={onRemoveSelection} />}
            </div>
            <div className="text-xs text-launcher-text/80 whitespace-pre-wrap break-words max-h-28 overflow-y-auto leading-relaxed">
              {truncate(contextInfo.selectedText, 500)}
            </div>
          </div>
        )}

        {/* Clipboard text */}
        {contextInfo.hasClipboard && contextInfo.clipboardText && (
          <div className="px-4 py-2.5 border-b border-launcher-border/20 hover:bg-launcher-hover/30 transition-colors">
            <div className="flex items-center justify-between mb-1">
              <div className="flex items-center gap-1.5">
                <span className="inline-flex items-center px-1.5 py-0.5 rounded text-[9px] font-medium bg-blue-500/20 text-blue-300 border border-blue-500/30">
                  CLIP
                </span>
                <span className="text-[10px] text-launcher-muted">Clipboard</span>
              </div>
              {onRemoveClipboard && <RemoveButton onClick={onRemoveClipboard} />}
            </div>
            <div className="text-xs text-launcher-text/80 whitespace-pre-wrap break-words max-h-28 overflow-y-auto leading-relaxed">
              {truncate(contextInfo.clipboardText, 500)}
            </div>
          </div>
        )}

        {/* Pasted images */}
        {images.map((img) => (
          <div
            key={img.id}
            className="px-4 py-2.5 border-b border-launcher-border/20 last:border-b-0 hover:bg-launcher-hover/30 transition-colors"
          >
            <div className="flex items-center justify-between mb-1">
              <div className="flex items-center gap-1.5">
                <span className="inline-flex items-center px-1.5 py-0.5 rounded text-[9px] font-medium bg-green-500/20 text-green-300 border border-green-500/30">
                  IMG
                </span>
                <span className="text-[10px] text-launcher-muted">{img.name}</span>
              </div>
              {onRemoveImage && <RemoveButton onClick={() => onRemoveImage(img.id)} />}
            </div>
            <img
              src={img.dataUrl}
              alt={img.name}
              className="w-full max-h-32 object-contain rounded border border-launcher-border/30 bg-black/20"
            />
          </div>
        ))}
      </div>
    </div>
  );
}
