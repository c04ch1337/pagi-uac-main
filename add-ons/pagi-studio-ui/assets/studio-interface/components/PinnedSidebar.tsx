import React from 'react';
import { X, Pin, Trash2, ExternalLink, Quote } from 'lucide-react';
import { Message } from '../types';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';

interface PinnedSidebarProps {
  isOpen: boolean;
  onClose: () => void;
  messages: Message[];
  onTogglePin: (id: string) => void;
}

const PinnedSidebar: React.FC<PinnedSidebarProps> = ({ isOpen, onClose, messages, onTogglePin }) => {
  if (!isOpen) return null;

  const pinnedMessages = messages.filter(m => m.isPinned);

  return (
    <div className="fixed inset-y-0 right-0 w-96 bg-white dark:bg-zinc-900 border-l border-zinc-200 dark:border-zinc-800 shadow-2xl transform transition-transform duration-300 z-50 flex flex-col">
      <div className="flex items-center justify-between p-4 border-b border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900 z-10">
        <h2 className="text-zinc-900 dark:text-zinc-100 font-medium flex items-center gap-2">
          <Pin size={18} className="text-orange-500" />
          Pinned Insights
        </h2>
        <button 
          onClick={onClose}
          className="text-zinc-400 hover:text-zinc-900 dark:hover:text-white transition-colors"
        >
          <X size={20} />
        </button>
      </div>

      <div className="flex-1 overflow-y-auto p-4 space-y-4 bg-zinc-50/50 dark:bg-black/20">
        {pinnedMessages.length === 0 ? (
           <div className="flex flex-col items-center justify-center h-64 text-zinc-400 text-center px-6 select-none">
             <div className="w-12 h-12 rounded-full bg-zinc-100 dark:bg-zinc-800 flex items-center justify-center mb-4 border border-zinc-200 dark:border-zinc-700">
                <Quote size={20} className="opacity-40" />
             </div>
             <p className="text-sm font-medium text-zinc-500 dark:text-zinc-400">No pinned messages</p>
             <p className="text-xs mt-2 text-zinc-400 dark:text-zinc-500 max-w-[200px] leading-relaxed">
               Pin important AGI responses to keep them accessible here for quick reference.
             </p>
           </div>
        ) : (
          pinnedMessages.map(msg => (
            <div key={msg.id} className="bg-white dark:bg-zinc-950 border border-zinc-200 dark:border-zinc-800 rounded-lg shadow-sm hover:shadow-md transition-shadow p-4 relative group">
               <div className="max-h-60 overflow-y-auto pr-1 scrollbar-thin">
                   {/* prose-sm is used for proper sizing, prose-zinc for colors */}
                   <ReactMarkdown 
                     remarkPlugins={[remarkGfm]} 
                     className="prose prose-sm dark:prose-invert prose-zinc max-w-none text-xs leading-relaxed"
                     components={{
                        // Override code block to be simple in sidebar
                        code: ({node, inline, className, children, ...props}: any) => (
                           inline 
                            ? <code className="bg-zinc-100 dark:bg-zinc-800 px-1 py-0.5 rounded text-zinc-800 dark:text-zinc-200 font-mono" {...props}>{children}</code>
                            : <pre className="bg-zinc-100 dark:bg-zinc-900 p-2 rounded text-zinc-800 dark:text-zinc-200 overflow-x-auto my-2 text-[10px]"><code {...props}>{children}</code></pre>
                        )
                     }}
                   >
                      {msg.content}
                   </ReactMarkdown>
               </div>
               <div className="flex justify-between items-center mt-3 pt-3 border-t border-zinc-100 dark:border-zinc-800/80">
                  <span className="text-[10px] text-zinc-400 font-mono uppercase tracking-wider">
                    {new Date(msg.timestamp).toLocaleString(undefined, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' })}
                  </span>
                  <button 
                      onClick={() => onTogglePin(msg.id)}
                      className="text-zinc-400 hover:text-red-500 dark:hover:text-red-400 transition-colors p-1.5 hover:bg-zinc-100 dark:hover:bg-zinc-900 rounded"
                      title="Unpin message"
                  >
                      <Trash2 size={14} />
                  </button>
               </div>
            </div>
          ))
        )}
      </div>
    </div>
  );
};
export default PinnedSidebar;