import { useState } from 'react';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faChevronDown, faChevronRight } from '@fortawesome/free-solid-svg-icons';

export default function CollapsibleSection({ title, icon, children, defaultOpen = true }) {
  const [isOpen, setIsOpen] = useState(defaultOpen);

  return (
    <div className="bg-slate-800 rounded-lg border border-slate-700 overflow-hidden">
      <button
        onClick={() => setIsOpen(!isOpen)}
        className="w-full px-6 py-4 flex items-center justify-between hover:bg-slate-750 transition-colors"
      >
        <div className="flex items-center">
          {icon && <FontAwesomeIcon icon={icon} className="mr-3 text-cyan-400" />}
          <h2 className="text-xl font-semibold text-slate-100">{title}</h2>
        </div>
        <FontAwesomeIcon
          icon={isOpen ? faChevronDown : faChevronRight}
          className="text-slate-400"
        />
      </button>
      
      {isOpen && (
        <div className="px-6 pb-6">
          {children}
        </div>
      )}
    </div>
  );
}
