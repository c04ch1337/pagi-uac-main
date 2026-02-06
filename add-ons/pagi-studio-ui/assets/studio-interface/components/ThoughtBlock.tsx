import React, { useState, useEffect } from 'react';
import { ChevronDown, ChevronRight, BrainCircuit, Lightbulb, Layers } from 'lucide-react';
import { ThoughtLayer } from '../types';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';

interface ThoughtBlockProps {
  thoughts: ThoughtLayer[];
  isExpanded?: boolean;
  onToggle?: () => void;
}

const ThoughtBlock: React.FC<ThoughtBlockProps> = ({ thoughts, isExpanded, onToggle }) => {
  const [internalExpanded, setInternalExpanded] = useState(true);
  
  // Determine if we are in controlled or uncontrolled mode
  const isControlled = isExpanded !== undefined;
  const showMain = isControlled ? isExpanded : internalExpanded;
  
  const handleToggle = () => {
      if (isControlled && onToggle) {
          onToggle();
      } else {
          setInternalExpanded(!internalExpanded);
      }
  };

  // Use a Set to track expanded IDs, allowing multiple sections to be open simultaneously
  const [expandedIds, setExpandedIds] = useState<Set<string>>(() => {
    const initial = new Set<string>();
    // Default to expanding the first thought if available
    if (thoughts.length > 0) {
      initial.add(thoughts[0].id);
    }
    // Also respect the 'expanded' property from the data model
    thoughts.forEach(t => {
      if (t.expanded) initial.add(t.id);
    });
    return initial;
  });

  // Sync with prop changes if needed (e.g. streaming adds new layers)
  useEffect(() => {
      if (thoughts.length > 0) {
          setExpandedIds(prev => {
              const next = new Set(prev);
              // Ensure at least one is expanded if it's the very first load
              if (prev.size === 0 && thoughts[0].expanded) {
                  next.add(thoughts[0].id);
              }
              return next;
          })
      }
  }, [thoughts.length]);

  if (!thoughts || thoughts.length === 0) return null;

  const toggleThought = (id: string) => {
    const newSet = new Set(expandedIds);
    if (newSet.has(id)) {
      newSet.delete(id);
    } else {
      newSet.add(id);
    }
    setExpandedIds(newSet);
  };

  return (
    <div className="mt-4 mb-2">
      {/* Main Header - Collapsible */}
      <button 
        onClick={handleToggle}
        className="flex items-center gap-2 mb-3 opacity-70 hover:opacity-100 transition-opacity w-full group focus:outline-none"
      >
         <div className={`transition-transform duration-200 text-zinc-400 group-hover:text-zinc-600 dark:group-hover:text-zinc-300 ${showMain ? 'rotate-90' : ''}`}>
             <ChevronRight size={12} />
         </div>
         <div className="h-px bg-zinc-200 dark:bg-zinc-800 flex-1 group-hover:bg-zinc-300 dark:group-hover:bg-zinc-700 transition-colors" />
         <span className="text-[10px] uppercase tracking-widest text-zinc-400 group-hover:text-zinc-600 dark:group-hover:text-zinc-300 font-medium flex items-center gap-1.5 transition-colors select-none">
            <BrainCircuit size={12} />
            Reasoning Process
            <span className="bg-zinc-100 dark:bg-zinc-800 px-1.5 py-0.5 rounded-full text-[9px] font-bold">
                {thoughts.length}
            </span>
         </span>
         <div className="h-px bg-zinc-200 dark:bg-zinc-800 flex-1 group-hover:bg-zinc-300 dark:group-hover:bg-zinc-700 transition-colors" />
      </button>
      
      {showMain && (
        <div className="space-y-2 animate-in fade-in slide-in-from-top-2 duration-300">
          {thoughts.map((thought, index) => {
            const isThoughtExpanded = expandedIds.has(thought.id);
            return (
              <div key={thought.id || index} className="bg-zinc-50 dark:bg-zinc-900/40 border border-zinc-200 dark:border-zinc-800/60 rounded-md overflow-hidden transition-all duration-200 group/item hover:border-zinc-300 dark:hover:border-zinc-700">
                <button
                  onClick={() => toggleThought(thought.id)}
                  className={`w-full flex items-center gap-3 px-3 py-2 text-xs font-medium transition-colors text-left focus:outline-none
                  ${isThoughtExpanded 
                      ? 'bg-zinc-100 dark:bg-zinc-800/60 text-zinc-900 dark:text-zinc-200' 
                      : 'text-zinc-500 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-200 hover:bg-zinc-100 dark:hover:bg-zinc-800/30'
                  }`}
                >
                  <div className={`transition-transform duration-200 ${isThoughtExpanded ? 'rotate-90' : 'rotate-0'} text-zinc-400`}>
                     <ChevronRight size={14} className="shrink-0" />
                  </div>
                  
                  <div className="flex items-center gap-2 flex-1 min-w-0">
                      {isThoughtExpanded ? <Lightbulb size={12} className="text-amber-500 shrink-0" /> : <Layers size={12} className="shrink-0 opacity-50" />}
                      <span className="uppercase tracking-wide opacity-90 truncate">{thought.title || `Step ${index + 1}`}</span>
                  </div>
                </button>
                
                {isThoughtExpanded && (
                  <div className="px-4 py-3 bg-white dark:bg-zinc-950/30 border-t border-zinc-200 dark:border-zinc-800/30">
                    <div className="prose prose-xs dark:prose-invert max-w-none text-zinc-600 dark:text-zinc-400 leading-relaxed font-mono text-[11px]">
                        <ReactMarkdown remarkPlugins={[remarkGfm]}>
                            {thought.content}
                        </ReactMarkdown>
                    </div>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
};

export default ThoughtBlock;