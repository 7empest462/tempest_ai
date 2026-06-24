import { useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { useStore } from '../store';
import { File, Folder, ChevronRight, FilePlus, FolderPlus, Edit2, Trash2 } from 'lucide-react';
import { playTabSwitchSound } from '../utils/audio';

export function FileExplorer() {
  const { currentPath, fileItems } = useStore();
  const [activeAction, setActiveAction] = useState<'createFile' | 'createFolder' | null>(null);
  const [editingItem, setEditingItem] = useState<any | null>(null);
  const [deletingItem, setDeletingItem] = useState<any | null>(null);
  const [inputValue, setInputValue] = useState('');

  const handleItemClick = (item: any) => {
    playTabSwitchSound();
    // If we are currently editing/deleting/creating, ignore clicks or reset
    if (editingItem || deletingItem || activeAction) {
      resetActions();
      return;
    }

    // @ts-ignore
    if (!window.sendNexus) return;

    if (item.name === '..') {
      const parts = currentPath.split('/');
      parts.pop();
      const parent = parts.join('/') || '.';
      // @ts-ignore
      window.sendNexus('ListFiles', { path: parent });
    } else {
      const fullPath = currentPath === '.' ? item.name : `${currentPath}/${item.name}`;
      if (item.is_dir) {
        // @ts-ignore
        window.sendNexus('ListFiles', { path: fullPath });
      } else {
        // @ts-ignore
        window.sendNexus('ReadFile', { path: fullPath });
      }
    }
  };

  const handleBack = () => {
    playTabSwitchSound();
    resetActions();
    // @ts-ignore
    if (window.sendNexus && currentPath !== '.' && currentPath !== '/') {
      const parts = currentPath.split('/');
      parts.pop();
      const parent = parts.join('/') || '.';
      // @ts-ignore
      window.sendNexus('ListFiles', { path: parent });
    }
  };

  const refreshFiles = () => {
    setTimeout(() => {
      // @ts-ignore
      if (window.sendNexus) window.sendNexus('ListFiles', { path: currentPath });
    }, 200);
  };

  const handleCreateFile = () => {
    setActiveAction('createFile');
    setInputValue('');
    setEditingItem(null);
    setDeletingItem(null);
  };

  const handleCreateFolder = () => {
    setActiveAction('createFolder');
    setInputValue('');
    setEditingItem(null);
    setDeletingItem(null);
  };

  const handleRenameClick = (e: React.MouseEvent, item: any) => {
    e.stopPropagation();
    if (item.name === '..') return;
    setEditingItem(item);
    setInputValue(item.name);
    setActiveAction(null);
    setDeletingItem(null);
  };

  const handleDeleteClick = (e: React.MouseEvent, item: any) => {
    e.stopPropagation();
    if (item.name === '..') return;
    setDeletingItem(item);
    setActiveAction(null);
    setEditingItem(null);
    setInputValue('');
  };

  const resetActions = () => {
    setActiveAction(null);
    setEditingItem(null);
    setDeletingItem(null);
    setInputValue('');
  };

  const confirmCreate = () => {
    if (!inputValue || !inputValue.trim()) return;
    const name = inputValue.trim();
    const fullPath = currentPath === '.' ? name : `${currentPath}/${name}`;

    // @ts-ignore
    if (window.sendNexus) {
      if (activeAction === 'createFile') {
        // @ts-ignore
        window.sendNexus('CreateFile', { path: fullPath });
      } else if (activeAction === 'createFolder') {
        // @ts-ignore
        window.sendNexus('CreateFolder', { path: fullPath });
      }
    }
    refreshFiles();
    resetActions();
  };

  const confirmRename = () => {
    if (!editingItem || !inputValue || !inputValue.trim() || inputValue.trim() === editingItem.name)
      return;
    const newName = inputValue.trim();
    const oldPath = currentPath === '.' ? editingItem.name : `${currentPath}/${editingItem.name}`;
    const newPath = currentPath === '.' ? newName : `${currentPath}/${newName}`;

    // @ts-ignore
    if (window.sendNexus) {
      // @ts-ignore
      window.sendNexus('RenameItem', { old_path: oldPath, new_path: newPath });
    }
    refreshFiles();
    resetActions();
  };

  const confirmDelete = () => {
    if (!deletingItem) return;
    const fullPath =
      currentPath === '.' ? deletingItem.name : `${currentPath}/${deletingItem.name}`;

    // @ts-ignore
    if (window.sendNexus) {
      // @ts-ignore
      window.sendNexus('DeleteItem', { path: fullPath });
    }
    refreshFiles();
    resetActions();
  };

  return (
    <div className="flex flex-col h-full select-none">
      <div className="flex items-center justify-between p-2 mb-2 bg-white/5 rounded-md text-xs font-mono">
        <div className="flex items-center gap-2 truncate text-muted-foreground">
          <button onClick={handleBack} className="hover:text-white transition-colors">
            &lt; BACK
          </button>
          <span className="truncate">{currentPath}</span>
        </div>
        <div className="flex items-center gap-1">
          <button
            onClick={handleCreateFile}
            className="p-1 hover:bg-white/10 rounded transition-colors"
            title="New File"
          >
            <FilePlus size={14} />
          </button>
          <button
            onClick={handleCreateFolder}
            className="p-1 hover:bg-white/10 rounded transition-colors"
            title="New Folder"
          >
            <FolderPlus size={14} />
          </button>
        </div>
      </div>

      <AnimatePresence>
        {activeAction && (
          <motion.div
            initial={{ opacity: 0, y: -10 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -10 }}
            className="p-2 mb-2 bg-white/5 border border-accent/25 rounded-md flex flex-col gap-2"
          >
            <div className="flex items-center gap-2">
              {activeAction === 'createFile' ? (
                <FilePlus size={14} className="text-accent" />
              ) : (
                <FolderPlus size={14} className="text-accent" />
              )}
              <span className="text-xs font-semibold uppercase text-muted-foreground">
                {activeAction === 'createFile' ? 'New File' : 'New Folder'}
              </span>
            </div>
            <div className="flex gap-2">
              <input
                type="text"
                value={inputValue}
                onChange={(e) => setInputValue(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') confirmCreate();
                  if (e.key === 'Escape') resetActions();
                }}
                placeholder={activeAction === 'createFile' ? 'filename.txt' : 'folder-name'}
                className="bg-black/20 border border-white/10 rounded px-2 py-1 text-sm text-white flex-1 font-mono focus:outline-none focus:border-accent/50"
                autoFocus
              />
            </div>
            <div className="flex justify-end gap-1.5 text-[10px] font-mono">
              <button
                onClick={confirmCreate}
                className="px-2 py-1 rounded bg-accent/20 text-accent hover:bg-accent/30 hover:text-white transition-all cursor-pointer font-bold"
              >
                CREATE
              </button>
              <button
                onClick={resetActions}
                className="px-2 py-1 rounded bg-white/5 text-muted-foreground hover:bg-white/10 hover:text-white transition-all cursor-pointer"
              >
                CANCEL
              </button>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      <div className="flex-1 overflow-y-auto">
        <AnimatePresence>
          {fileItems.length === 0 ? (
            <motion.p
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              className="text-muted-foreground text-sm p-2 text-center italic"
            >
              Empty directory
            </motion.p>
          ) : (
            fileItems.map((item, idx) => {
              const isEditing = editingItem && editingItem.name === item.name;
              const isDeleting = deletingItem && deletingItem.name === item.name;

              if (isEditing) {
                return (
                  <motion.div
                    key={item.name}
                    className="flex items-center gap-2 p-1 bg-white/5 border border-accent/20 rounded-md my-1"
                    onClick={(e) => e.stopPropagation()}
                  >
                    {item.is_dir ? (
                      <Folder size={14} className="text-accent/70 shrink-0" />
                    ) : (
                      <File size={14} className="shrink-0" />
                    )}
                    <input
                      type="text"
                      value={inputValue}
                      onChange={(e) => setInputValue(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter') confirmRename();
                        if (e.key === 'Escape') resetActions();
                      }}
                      className="bg-black/30 border border-white/10 rounded px-1.5 py-0.5 text-xs text-white flex-1 font-mono focus:outline-none focus:border-accent/40"
                      autoFocus
                    />
                    <button
                      onClick={confirmRename}
                      className="text-[9px] font-mono font-bold bg-accent/20 text-accent px-1.5 py-0.5 rounded hover:bg-accent/30 cursor-pointer"
                    >
                      SAVE
                    </button>
                    <button
                      onClick={resetActions}
                      className="text-[9px] font-mono bg-white/5 text-muted-foreground px-1.5 py-0.5 rounded hover:bg-white/10 cursor-pointer"
                    >
                      X
                    </button>
                  </motion.div>
                );
              }

              if (isDeleting) {
                return (
                  <motion.div
                    key={item.name}
                    className="flex items-center justify-between gap-2 p-1.5 bg-red-950/20 border border-red-500/30 rounded-md my-1 text-xs"
                    onClick={(e) => e.stopPropagation()}
                  >
                    <span className="text-red-400 font-medium truncate flex-1">
                      Delete {item.name}?
                    </span>
                    <div className="flex gap-1 shrink-0">
                      <button
                        onClick={confirmDelete}
                        className="text-[9px] font-mono font-bold bg-red-500/20 text-red-300 px-2 py-0.5 rounded hover:bg-red-500/30 cursor-pointer"
                      >
                        YES
                      </button>
                      <button
                        onClick={resetActions}
                        className="text-[9px] font-mono bg-white/5 text-muted-foreground px-2 py-0.5 rounded hover:bg-white/10 cursor-pointer"
                      >
                        NO
                      </button>
                    </div>
                  </motion.div>
                );
              }

              return (
                <motion.div
                  key={item.name}
                  onClick={() => handleItemClick(item)}
                  initial={{ opacity: 0, x: -10 }}
                  animate={{ opacity: 1, x: 0 }}
                  transition={{ delay: idx * 0.05 }}
                  className="flex items-center gap-2 p-2 hover:bg-accent/10 rounded-md cursor-pointer group text-sm text-muted-foreground hover:text-white transition-colors"
                >
                  {item.is_dir ? (
                    <ChevronRight
                      size={14}
                      className="group-hover:text-accent transition-colors shrink-0"
                    />
                  ) : (
                    <span className="w-3.5 shrink-0" />
                  )}
                  {item.is_dir ? (
                    <Folder size={14} className="text-accent/70 shrink-0" />
                  ) : (
                    <File size={14} className="shrink-0" />
                  )}
                  <span className="truncate flex-1">{item.name}</span>

                  {item.name !== '..' && (
                    <div className="opacity-0 group-hover:opacity-100 flex items-center gap-1 transition-opacity">
                      <button
                        onClick={(e) => handleRenameClick(e, item)}
                        className="p-1 hover:bg-white/10 rounded text-muted-foreground hover:text-white transition-colors"
                        title="Rename"
                      >
                        <Edit2 size={13} />
                      </button>
                      <button
                        onClick={(e) => handleDeleteClick(e, item)}
                        className="p-1 hover:bg-red-500/20 rounded text-red-400 hover:text-red-300 transition-colors"
                        title="Delete"
                      >
                        <Trash2 size={13} />
                      </button>
                    </div>
                  )}
                </motion.div>
              );
            })
          )}
        </AnimatePresence>
      </div>
    </div>
  );
}
