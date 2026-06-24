import { a as e, r as t } from "./rolldown-runtime-Cyuzqnbw.js";
import { at as n, ct as r, it as i, nt as a, ot as o, rt as s, st as c, t as l, tt as u } from "./vendor-COyXhJkX.js";
import { n as d, t as f } from "./vendor-xterm-BIbaqHeJ.js";
import { n as p, t as m } from "./vendor-framer-motion-B9jWlAXH.js";
import { A as h, B as g, C as _, D as v, E as y, F as ee, G as b, H as te, I as x, K as S, L as C, M as ne, N as re, O as ie, P as ae, R as oe, S as se, T as ce, U as le, V as ue, W as de, _ as fe, a as pe, b as me, c as he, d as ge, f as _e, g as ve, h as ye, i as be, j as xe, k as Se, l as Ce, m as we, n as Te, o as Ee, p as De, r as Oe, s as ke, t as Ae, u as je, v as Me, w as Ne, x as Pe, y as Fe, z as Ie } from "./vendor-lucide-3AWDZkyr.js";
import { n as Le, r as Re, t as ze } from "./vendor-monaco-DMqCiMS9.js";
(async ()=>{
    (function() {
        let e = document.createElement(`link`).relList;
        if (e && e.supports && e.supports(`modulepreload`)) return;
        for (let e of document.querySelectorAll(`link[rel="modulepreload"]`))n(e);
        new MutationObserver((e)=>{
            for (let t of e)if (t.type === `childList`) for (let e of t.addedNodes)e.tagName === `LINK` && e.rel === `modulepreload` && n(e);
        }).observe(document, {
            childList: !0,
            subtree: !0
        });
        function t(e) {
            let t = {};
            return e.integrity && (t.integrity = e.integrity), e.referrerPolicy && (t.referrerPolicy = e.referrerPolicy), e.crossOrigin === `use-credentials` ? t.credentials = `include` : e.crossOrigin === `anonymous` ? t.credentials = `omit` : t.credentials = `same-origin`, t;
        }
        function n(e) {
            if (e.ep) return;
            e.ep = !0;
            let n = t(e);
            fetch(e.href, n);
        }
    })();
    var w = e(r(), 1), Be = e(c(), 1), T = a()(u((e)=>({
            isConnected: !1,
            engineStatus: `Initializing...`,
            plannerModel: `--`,
            executorModel: `--`,
            verifierModel: `--`,
            setConnected: (t)=>e({
                    isConnected: t
                }),
            setEngineStatus: (t)=>e({
                    engineStatus: t
                }),
            setBackendInfo: (t, n, r, i)=>e({
                    engineStatus: t,
                    plannerModel: n,
                    executorModel: r,
                    verifierModel: i
                }),
            cpu: 0,
            gpu: 0,
            ram: `--`,
            tps: `idle`,
            ctxUsed: 0,
            ctxTotal: 32768,
            kvCacheHitPct: null,
            planningDurationMs: null,
            executingDurationMs: null,
            verifyingDurationMs: null,
            setMetrics: (t, n, r)=>e({
                    cpu: t,
                    gpu: n,
                    ram: r
                }),
            setTps: (t)=>e({
                    tps: t
                }),
            setCtxUsed: (t)=>e({
                    ctxUsed: t
                }),
            setCtxTotal: (t)=>e({
                    ctxTotal: t
                }),
            setKvCacheHitPct: (t)=>e((e)=>({
                        kvCacheHitPct: t ?? e.kvCacheHitPct
                    })),
            setPhaseDurations: (t, n, r)=>e((e)=>({
                        planningDurationMs: t ?? e.planningDurationMs,
                        executingDurationMs: n ?? e.executingDurationMs,
                        verifyingDurationMs: r ?? e.verifyingDurationMs
                    })),
            agentPhase: `Idle`,
            currentTask: `--`,
            activeTools: [],
            setAgentPhase: (t)=>e({
                    agentPhase: t
                }),
            setCurrentTask: (t)=>e({
                    currentTask: t
                }),
            setActiveTools: (t)=>e({
                    activeTools: t
                }),
            messages: [
                {
                    id: `init`,
                    role: `system`,
                    content: `🌪️ [SYSTEM]: Neural link established. Environment grounded.`
                }
            ],
            isStreaming: !1,
            streamAccumulator: ``,
            safeModeRequest: null,
            askUserRequest: null,
            addMessage: (t)=>e((e)=>({
                        messages: [
                            ...e.messages,
                            t
                        ]
                    })),
            setMessages: (t)=>e({
                    messages: t
                }),
            updateLastMessage: (t)=>e((e)=>{
                    let n = [
                        ...e.messages
                    ];
                    return n.length > 0 && (n[n.length - 1].content = t), {
                        messages: n
                    };
                }),
            setStreaming: (t)=>e((e)=>({
                        isStreaming: t,
                        kvCacheHitPct: e.kvCacheHitPct,
                        planningDurationMs: e.planningDurationMs,
                        executingDurationMs: e.executingDurationMs,
                        verifyingDurationMs: e.verifyingDurationMs
                    })),
            appendStreamContent: (t)=>e((e)=>({
                        streamAccumulator: e.streamAccumulator + t
                    })),
            commitStream: ()=>e((e)=>!e.streamAccumulator && !e.reasoningAccumulator && e.currentToolResults.length === 0 ? {
                        isStreaming: !1
                    } : {
                        messages: [
                            ...e.messages,
                            {
                                id: Date.now().toString(),
                                role: `ai`,
                                content: e.streamAccumulator,
                                reasoning: e.reasoningAccumulator,
                                tools: e.currentToolResults
                            }
                        ],
                        streamAccumulator: ``,
                        reasoningAccumulator: ``,
                        currentToolResults: [],
                        isStreaming: !1
                    }),
            setSafeModeRequest: (t)=>e({
                    safeModeRequest: t
                }),
            setAskUserRequest: (t)=>e({
                    askUserRequest: t
                }),
            memories: [],
            setMemories: (t)=>e({
                    memories: t
                }),
            activeToolExecutions: [],
            addActiveToolExecution: (t, n)=>e((e)=>{
                    let r = {
                        id: `${t}-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`,
                        name: t,
                        args: n,
                        status: `running`,
                        progress: 15
                    };
                    return {
                        activeToolExecutions: [
                            ...e.activeToolExecutions,
                            r
                        ]
                    };
                }),
            updateActiveToolExecution: (t, n, r, i)=>e((e)=>{
                    let a = [
                        ...e.activeToolExecutions
                    ], o = -1;
                    return n && (o = a.findIndex((e)=>e.name === t && e.status === `running` && e.args === n)), o === -1 && (o = a.findIndex((e)=>e.name === t && e.status === `running`)), o === -1 && (o = a.findIndex((e)=>e.name === t)), o === -1 ? a.push({
                        id: `${t}-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`,
                        name: t,
                        args: n,
                        status: r,
                        output: i,
                        progress: 100
                    }) : a[o] = {
                        ...a[o],
                        status: r,
                        output: i,
                        progress: 100
                    }, {
                        activeToolExecutions: a
                    };
                }),
            clearActiveToolExecutions: ()=>e({
                    activeToolExecutions: []
                }),
            turnReviewRequest: null,
            setTurnReviewRequest: (t)=>e({
                    turnReviewRequest: t
                }),
            currentPath: `/`,
            fileItems: [],
            setExplorer: (t, n)=>e({
                    currentPath: t,
                    fileItems: n
                }),
            activeFile: null,
            setActiveFile: (t)=>e({
                    activeFile: t,
                    isFileEditable: !1
                }),
            updateActiveFileContent: (t)=>e((e)=>({
                        activeFile: e.activeFile ? {
                            ...e.activeFile,
                            content: t
                        } : null
                    })),
            isFileEditable: !1,
            setFileEditable: (t)=>e({
                    isFileEditable: t
                }),
            isEditorFocused: !1,
            setEditorFocused: (t)=>e({
                    isEditorFocused: t
                }),
            isTerminalOpen: !1,
            setTerminalOpen: (t)=>e({
                    isTerminalOpen: t
                }),
            backgroundIntensity: `subtle`,
            setBackgroundIntensity: (t)=>e({
                    backgroundIntensity: t
                }),
            sliderAggressiveCareful: .3,
            sliderCreativePrecise: .4,
            sliderFastThorough: .3,
            activeRole: `pair-programmer`,
            contextLimit: 32768,
            muteSounds: !1,
            setSliderAggressiveCareful: (t)=>e({
                    sliderAggressiveCareful: t
                }),
            setSliderCreativePrecise: (t)=>e({
                    sliderCreativePrecise: t
                }),
            setSliderFastThorough: (t)=>e({
                    sliderFastThorough: t
                }),
            setActiveRole: (t)=>e({
                    activeRole: t
                }),
            setContextLimit: (t)=>e({
                    contextLimit: t
                }),
            setMuteSounds: (t)=>e({
                    muteSounds: t
                }),
            activeTab: `files`,
            setActiveTab: (t)=>e({
                    activeTab: t
                }),
            chatViewMode: `timeline`,
            setChatViewMode: (t)=>e({
                    chatViewMode: t
                }),
            reasoningAccumulator: ``,
            appendReasoningContent: (t)=>e((e)=>({
                        reasoningAccumulator: e.reasoningAccumulator + t
                    })),
            clearReasoning: ()=>e({
                    reasoningAccumulator: ``
                }),
            currentToolResults: [],
            addToolResult: (t)=>e((e)=>({
                        currentToolResults: [
                            ...e.currentToolResults,
                            t
                        ]
                    })),
            clearToolResults: ()=>e({
                    currentToolResults: []
                }),
            searchResults: [],
            isSearching: !1,
            setSearchResults: (t)=>e({
                    searchResults: t
                }),
            setSearching: (t)=>e({
                    isSearching: t
                })
        }), {
        name: `tempest-settings`,
        partialize: (e)=>({
                isTerminalOpen: e.isTerminalOpen,
                backgroundIntensity: e.backgroundIntensity,
                sliderAggressiveCareful: e.sliderAggressiveCareful,
                sliderCreativePrecise: e.sliderCreativePrecise,
                sliderFastThorough: e.sliderFastThorough,
                activeRole: e.activeRole,
                contextLimit: e.contextLimit,
                muteSounds: e.muteSounds
            })
    })), E = o();
    function Ve() {
        let e = (0, w.useRef)(null), t = (0, w.useRef)(null), n = (0, w.useRef)(new f);
        return (0, w.useEffect)(()=>{
            if (!e.current || t.current) return;
            t.current = new d({
                theme: {
                    background: `transparent`,
                    foreground: `#a0a0c0`,
                    cursor: `#00f2ff`,
                    cursorAccent: `#000000`,
                    selectionBackground: `rgba(0, 242, 255, 0.3)`
                },
                fontFamily: `"JetBrains Mono", monospace`,
                fontSize: 13,
                cursorBlink: !0,
                scrollback: 5e3,
                convertEol: !0
            }), t.current.loadAddon(n.current), t.current.open(e.current), n.current.fit(), t.current.writeln(`\x1B[1;36m🌪️ Terminal Subsystem Online.\x1B[0m`), window.sendNexus && window.sendNexus(`TerminalSpawn`, {});
            let r = t.current.onData((e)=>{
                window.sendNexus && window.sendNexus(`TerminalInput`, {
                    data: e
                });
            }), i = t.current.onResize(({ cols: e, rows: t })=>{
                window.sendNexus && window.sendNexus(`TerminalResize`, {
                    cols: e,
                    rows: t
                });
            }), a = (e)=>{
                let n = e;
                t.current?.write(n.detail);
            };
            window.addEventListener(`terminal-output`, a);
            let o = new ResizeObserver(()=>{
                requestAnimationFrame(()=>{
                    try {
                        n.current.fit();
                    } catch  {}
                });
            });
            return o.observe(e.current), ()=>{
                r.dispose(), i.dispose(), window.removeEventListener(`terminal-output`, a), o.disconnect(), t.current?.dispose(), t.current = null;
            };
        }, []), (0, E.jsx)(`div`, {
            ref: e,
            className: `h-full w-full p-2`
        });
    }
    var D = null, O = ()=>(D ||= new (window.AudioContext || window.webkitAudioContext), D.state === `suspended` && D.resume(), D), k = .15, A = ()=>!T.getState().muteSounds, He = ()=>{
        if (!A()) return;
        let e = O(), t = e.currentTime, n = e.createOscillator(), r = e.createGain();
        n.type = `triangle`, n.frequency.setValueAtTime(800, t), n.frequency.exponentialRampToValueAtTime(1200, t + .1), r.gain.setValueAtTime(0, t), r.gain.linearRampToValueAtTime(k, t + .02), r.gain.exponentialRampToValueAtTime(.01, t + .15), n.connect(r), r.connect(e.destination), n.start(t), n.stop(t + .2);
    }, Ue = ()=>{
        if (!A()) return;
        let e = O(), t = e.currentTime, n = e.createOscillator(), r = e.createOscillator(), i = e.createGain();
        n.type = `sine`, r.type = `sine`, n.frequency.setValueAtTime(880, t), r.frequency.setValueAtTime(1760, t), i.gain.setValueAtTime(0, t), i.gain.linearRampToValueAtTime(k * .8, t + .05), i.gain.exponentialRampToValueAtTime(.01, t + .6), n.connect(i), r.connect(i), i.connect(e.destination), n.start(t), r.start(t), n.stop(t + .7), r.stop(t + .7);
    }, We = ()=>{
        if (!A()) return;
        let e = O(), t = e.currentTime, n = e.createOscillator(), r = e.createGain();
        n.type = `sawtooth`, n.frequency.setValueAtTime(150, t), n.frequency.exponentialRampToValueAtTime(80, t + .3), r.gain.setValueAtTime(0, t), r.gain.linearRampToValueAtTime(k, t + .02), r.gain.setValueAtTime(k, t + .1), r.gain.setValueAtTime(0, t + .11), r.gain.setValueAtTime(k * .8, t + .15), r.gain.exponentialRampToValueAtTime(.01, t + .4), n.connect(r), r.connect(e.destination), n.start(t), n.stop(t + .5);
    }, Ge = ()=>{
        if (!A()) return;
        let e = O(), t = e.currentTime, n = e.createOscillator(), r = e.createGain();
        n.type = `sine`, n.frequency.setValueAtTime(1200, t), n.frequency.exponentialRampToValueAtTime(2e3, t + .3), r.gain.setValueAtTime(0, t), r.gain.linearRampToValueAtTime(k * .5, t + .1), r.gain.exponentialRampToValueAtTime(.01, t + .6), n.connect(r), r.connect(e.destination), n.start(t), n.stop(t + .7);
    }, Ke = ()=>{
        if (!A()) return;
        let e = O(), t = e.currentTime, n = e.createOscillator(), r = e.createGain();
        n.type = `square`, n.frequency.setValueAtTime(150, t), n.frequency.exponentialRampToValueAtTime(40, t + .1), r.gain.setValueAtTime(0, t), r.gain.linearRampToValueAtTime(k * .8, t + .01), r.gain.exponentialRampToValueAtTime(.01, t + .15);
        let i = e.createBiquadFilter();
        i.type = `lowpass`, i.frequency.value = 400, n.connect(i), i.connect(r), r.connect(e.destination), n.start(t), n.stop(t + .2);
    }, qe = 0, Je = ()=>{
        if (!A()) return;
        let e = Date.now();
        if (e - qe < 40) return;
        qe = e;
        let t = O(), n = t.currentTime, r = t.createOscillator(), i = t.createGain();
        r.type = `square`, r.frequency.value = 2e3 + Math.random() * 2e3, i.gain.setValueAtTime(k * .1, n), i.gain.exponentialRampToValueAtTime(.01, n + .05), r.connect(i), i.connect(t.destination), r.start(n), r.stop(n + .06);
    }, j = ()=>{
        if (!A()) return;
        let e = O(), t = e.currentTime, n = e.createOscillator(), r = e.createGain();
        n.type = `sine`, n.frequency.setValueAtTime(1e3, t), n.frequency.exponentialRampToValueAtTime(600, t + .05), r.gain.setValueAtTime(0, t), r.gain.linearRampToValueAtTime(k * .4, t + .01), r.gain.exponentialRampToValueAtTime(.01, t + .05), n.connect(r), r.connect(e.destination), n.start(t), n.stop(t + .06);
    }, Ye = 0, Xe = ()=>{
        if (!A()) return;
        let e = Date.now();
        if (e - Ye < 100) return;
        Ye = e;
        let t = O(), n = t.currentTime, r = t.createOscillator(), i = t.createGain();
        r.type = `triangle`, r.frequency.value = 100 + Math.random() * 50, i.gain.setValueAtTime(0, n), i.gain.linearRampToValueAtTime(k * .2, n + .02), i.gain.exponentialRampToValueAtTime(.01, n + .1), r.connect(i), i.connect(t.destination), r.start(n), r.stop(n + .12);
    };
    function Ze() {
        let { currentPath: e, fileItems: t } = T(), [n, r] = (0, w.useState)(null), [i, a] = (0, w.useState)(null), [o, s] = (0, w.useState)(null), [c, l] = (0, w.useState)(``), u = (t)=>{
            if (j(), i || o || n) {
                x();
                return;
            }
            if (window.sendNexus) if (t.name === `..`) {
                let t = e.split(`/`);
                t.pop();
                let n = t.join(`/`) || `.`;
                window.sendNexus(`ListFiles`, {
                    path: n
                });
            } else {
                let n = e === `.` ? t.name : `${e}/${t.name}`;
                t.is_dir ? window.sendNexus(`ListFiles`, {
                    path: n
                }) : window.sendNexus(`ReadFile`, {
                    path: n
                });
            }
        }, d = ()=>{
            if (j(), x(), window.sendNexus && e !== `.` && e !== `/`) {
                let t = e.split(`/`);
                t.pop();
                let n = t.join(`/`) || `.`;
                window.sendNexus(`ListFiles`, {
                    path: n
                });
            }
        }, f = ()=>{
            setTimeout(()=>{
                window.sendNexus && window.sendNexus(`ListFiles`, {
                    path: e
                });
            }, 200);
        }, g = ()=>{
            r(`createFile`), l(``), a(null), s(null);
        }, _ = ()=>{
            r(`createFolder`), l(``), a(null), s(null);
        }, ee = (e, t)=>{
            e.stopPropagation(), t.name !== `..` && (a(t), l(t.name), r(null), s(null));
        }, b = (e, t)=>{
            e.stopPropagation(), t.name !== `..` && (s(t), r(null), a(null), l(``));
        }, x = ()=>{
            r(null), a(null), s(null), l(``);
        }, S = ()=>{
            if (!c || !c.trim()) return;
            let t = c.trim(), r = e === `.` ? t : `${e}/${t}`;
            window.sendNexus && (n === `createFile` ? window.sendNexus(`CreateFile`, {
                path: r
            }) : n === `createFolder` && window.sendNexus(`CreateFolder`, {
                path: r
            })), f(), x();
        }, C = ()=>{
            if (!i || !c || !c.trim() || c.trim() === i.name) return;
            let t = c.trim(), n = e === `.` ? i.name : `${e}/${i.name}`, r = e === `.` ? t : `${e}/${t}`;
            window.sendNexus && window.sendNexus(`RenameItem`, {
                old_path: n,
                new_path: r
            }), f(), x();
        }, ne = ()=>{
            if (!o) return;
            let t = e === `.` ? o.name : `${e}/${o.name}`;
            window.sendNexus && window.sendNexus(`DeleteItem`, {
                path: t
            }), f(), x();
        };
        return (0, E.jsxs)(`div`, {
            className: `flex flex-col h-full select-none`,
            children: [
                (0, E.jsxs)(`div`, {
                    className: `flex items-center justify-between p-2 mb-2 bg-white/5 rounded-md text-xs font-mono`,
                    children: [
                        (0, E.jsxs)(`div`, {
                            className: `flex items-center gap-2 truncate text-muted-foreground`,
                            children: [
                                (0, E.jsx)(`button`, {
                                    onClick: d,
                                    className: `hover:text-white transition-colors`,
                                    children: `< BACK`
                                }),
                                (0, E.jsx)(`span`, {
                                    className: `truncate`,
                                    children: e
                                })
                            ]
                        }),
                        (0, E.jsxs)(`div`, {
                            className: `flex items-center gap-1`,
                            children: [
                                (0, E.jsx)(`button`, {
                                    onClick: g,
                                    className: `p-1 hover:bg-white/10 rounded transition-colors`,
                                    title: `New File`,
                                    children: (0, E.jsx)(h, {
                                        size: 14
                                    })
                                }),
                                (0, E.jsx)(`button`, {
                                    onClick: _,
                                    className: `p-1 hover:bg-white/10 rounded transition-colors`,
                                    title: `New Folder`,
                                    children: (0, E.jsx)(v, {
                                        size: 14
                                    })
                                })
                            ]
                        })
                    ]
                }),
                (0, E.jsx)(p, {
                    children: n && (0, E.jsxs)(m.div, {
                        initial: {
                            opacity: 0,
                            y: -10
                        },
                        animate: {
                            opacity: 1,
                            y: 0
                        },
                        exit: {
                            opacity: 0,
                            y: -10
                        },
                        className: `p-2 mb-2 bg-white/5 border border-accent/25 rounded-md flex flex-col gap-2`,
                        children: [
                            (0, E.jsxs)(`div`, {
                                className: `flex items-center gap-2`,
                                children: [
                                    n === `createFile` ? (0, E.jsx)(h, {
                                        size: 14,
                                        className: `text-accent`
                                    }) : (0, E.jsx)(v, {
                                        size: 14,
                                        className: `text-accent`
                                    }),
                                    (0, E.jsx)(`span`, {
                                        className: `text-xs font-semibold uppercase text-muted-foreground`,
                                        children: n === `createFile` ? `New File` : `New Folder`
                                    })
                                ]
                            }),
                            (0, E.jsx)(`div`, {
                                className: `flex gap-2`,
                                children: (0, E.jsx)(`input`, {
                                    type: `text`,
                                    value: c,
                                    onChange: (e)=>l(e.target.value),
                                    onKeyDown: (e)=>{
                                        e.key === `Enter` && S(), e.key === `Escape` && x();
                                    },
                                    placeholder: n === `createFile` ? `filename.txt` : `folder-name`,
                                    className: `bg-black/20 border border-white/10 rounded px-2 py-1 text-sm text-white flex-1 font-mono focus:outline-none focus:border-accent/50`,
                                    autoFocus: !0
                                })
                            }),
                            (0, E.jsxs)(`div`, {
                                className: `flex justify-end gap-1.5 text-[10px] font-mono`,
                                children: [
                                    (0, E.jsx)(`button`, {
                                        onClick: S,
                                        className: `px-2 py-1 rounded bg-accent/20 text-accent hover:bg-accent/30 hover:text-white transition-all cursor-pointer font-bold`,
                                        children: `CREATE`
                                    }),
                                    (0, E.jsx)(`button`, {
                                        onClick: x,
                                        className: `px-2 py-1 rounded bg-white/5 text-muted-foreground hover:bg-white/10 hover:text-white transition-all cursor-pointer`,
                                        children: `CANCEL`
                                    })
                                ]
                            })
                        ]
                    })
                }),
                (0, E.jsx)(`div`, {
                    className: `flex-1 overflow-y-auto`,
                    children: (0, E.jsx)(p, {
                        children: t.length === 0 ? (0, E.jsx)(m.p, {
                            initial: {
                                opacity: 0
                            },
                            animate: {
                                opacity: 1
                            },
                            className: `text-muted-foreground text-sm p-2 text-center italic`,
                            children: `Empty directory`
                        }) : t.map((e, t)=>{
                            let n = i && i.name === e.name, r = o && o.name === e.name;
                            return n ? (0, E.jsxs)(m.div, {
                                className: `flex items-center gap-2 p-1 bg-white/5 border border-accent/20 rounded-md my-1`,
                                onClick: (e)=>e.stopPropagation(),
                                children: [
                                    e.is_dir ? (0, E.jsx)(y, {
                                        size: 14,
                                        className: `text-accent/70 shrink-0`
                                    }) : (0, E.jsx)(ie, {
                                        size: 14,
                                        className: `shrink-0`
                                    }),
                                    (0, E.jsx)(`input`, {
                                        type: `text`,
                                        value: c,
                                        onChange: (e)=>l(e.target.value),
                                        onKeyDown: (e)=>{
                                            e.key === `Enter` && C(), e.key === `Escape` && x();
                                        },
                                        className: `bg-black/30 border border-white/10 rounded px-1.5 py-0.5 text-xs text-white flex-1 font-mono focus:outline-none focus:border-accent/40`,
                                        autoFocus: !0
                                    }),
                                    (0, E.jsx)(`button`, {
                                        onClick: C,
                                        className: `text-[9px] font-mono font-bold bg-accent/20 text-accent px-1.5 py-0.5 rounded hover:bg-accent/30 cursor-pointer`,
                                        children: `SAVE`
                                    }),
                                    (0, E.jsx)(`button`, {
                                        onClick: x,
                                        className: `text-[9px] font-mono bg-white/5 text-muted-foreground px-1.5 py-0.5 rounded hover:bg-white/10 cursor-pointer`,
                                        children: `X`
                                    })
                                ]
                            }, e.name) : r ? (0, E.jsxs)(m.div, {
                                className: `flex items-center justify-between gap-2 p-1.5 bg-red-950/20 border border-red-500/30 rounded-md my-1 text-xs`,
                                onClick: (e)=>e.stopPropagation(),
                                children: [
                                    (0, E.jsxs)(`span`, {
                                        className: `text-red-400 font-medium truncate flex-1`,
                                        children: [
                                            `Delete `,
                                            e.name,
                                            `?`
                                        ]
                                    }),
                                    (0, E.jsxs)(`div`, {
                                        className: `flex gap-1 shrink-0`,
                                        children: [
                                            (0, E.jsx)(`button`, {
                                                onClick: ne,
                                                className: `text-[9px] font-mono font-bold bg-red-500/20 text-red-300 px-2 py-0.5 rounded hover:bg-red-500/30 cursor-pointer`,
                                                children: `YES`
                                            }),
                                            (0, E.jsx)(`button`, {
                                                onClick: x,
                                                className: `text-[9px] font-mono bg-white/5 text-muted-foreground px-2 py-0.5 rounded hover:bg-white/10 cursor-pointer`,
                                                children: `NO`
                                            })
                                        ]
                                    })
                                ]
                            }, e.name) : (0, E.jsxs)(m.div, {
                                onClick: ()=>u(e),
                                initial: {
                                    opacity: 0,
                                    x: -10
                                },
                                animate: {
                                    opacity: 1,
                                    x: 0
                                },
                                transition: {
                                    delay: t * .05
                                },
                                className: `flex items-center gap-2 p-2 hover:bg-accent/10 rounded-md cursor-pointer group text-sm text-muted-foreground hover:text-white transition-colors`,
                                children: [
                                    e.is_dir ? (0, E.jsx)(te, {
                                        size: 14,
                                        className: `group-hover:text-accent transition-colors shrink-0`
                                    }) : (0, E.jsx)(`span`, {
                                        className: `w-3.5 shrink-0`
                                    }),
                                    e.is_dir ? (0, E.jsx)(y, {
                                        size: 14,
                                        className: `text-accent/70 shrink-0`
                                    }) : (0, E.jsx)(ie, {
                                        size: 14,
                                        className: `shrink-0`
                                    }),
                                    (0, E.jsx)(`span`, {
                                        className: `truncate flex-1`,
                                        children: e.name
                                    }),
                                    e.name !== `..` && (0, E.jsxs)(`div`, {
                                        className: `opacity-0 group-hover:opacity-100 flex items-center gap-1 transition-opacity`,
                                        children: [
                                            (0, E.jsx)(`button`, {
                                                onClick: (t)=>ee(t, e),
                                                className: `p-1 hover:bg-white/10 rounded text-muted-foreground hover:text-white transition-colors`,
                                                title: `Rename`,
                                                children: (0, E.jsx)(se, {
                                                    size: 13
                                                })
                                            }),
                                            (0, E.jsx)(`button`, {
                                                onClick: (t)=>b(t, e),
                                                className: `p-1 hover:bg-red-500/20 rounded text-red-400 hover:text-red-300 transition-colors`,
                                                title: `Delete`,
                                                children: (0, E.jsx)(he, {
                                                    size: 13
                                                })
                                            })
                                        ]
                                    })
                                ]
                            }, e.name);
                        })
                    })
                })
            ]
        });
    }
    function Qe() {
        let { messages: e, isStreaming: t, addMessage: n, streamAccumulator: r, reasoningAccumulator: i, currentToolResults: a, activeFile: o, agentPhase: s } = T(), [c, l] = (0, w.useState)(``), u = (0, w.useRef)(null);
        (0, w.useEffect)(()=>{
            u.current && (u.current.scrollTop = u.current.scrollHeight);
        }, [
            e,
            t,
            r,
            i,
            a,
            s
        ]);
        let d = ()=>{
            if (!(!c.trim() || t)) {
                if (n({
                    id: Date.now().toString(),
                    role: `user`,
                    content: c
                }), window.sendNexus) {
                    let e;
                    o && (e = `${o.name}\n\nFile Contents:\n\`\`\`${o.ext}\n${o.content}\n\`\`\``);
                    let t = T.getState(), n = Math.max(.01, t.sliderCreativePrecise * 1 + t.sliderAggressiveCareful * .6 + t.sliderFastThorough * .4);
                    window.sendNexus(`Chat`, {
                        message: c,
                        editor_context: e,
                        temperature: n,
                        context_limit: t.contextLimit,
                        role: t.activeRole
                    }), T.getState().clearActiveToolExecutions(), T.getState().setStreaming(!0);
                }
                l(``);
            }
        };
        return (0, E.jsxs)(`div`, {
            className: `flex flex-col h-full w-full`,
            children: [
                (0, E.jsxs)(`div`, {
                    ref: u,
                    className: `flex-1 overflow-y-auto p-4 flex flex-col gap-4`,
                    children: [
                        (0, E.jsx)(p, {
                            children: e.map((e)=>(0, E.jsxs)(m.div, {
                                    initial: {
                                        opacity: 0,
                                        y: 10
                                    },
                                    animate: {
                                        opacity: 1,
                                        y: 0
                                    },
                                    className: `max-w-[85%] p-4 rounded-xl text-sm leading-relaxed ${e.role === `system` ? `bg-accent/10 border border-accent/20 font-mono self-start` : e.role === `ai` ? `bg-white/5 border border-white/10 self-start` : `bg-[rgba(112,0,255,0.2)] border border-[rgba(112,0,255,0.4)] self-end`}`,
                                    children: [
                                        e.role === `system` && (0, E.jsx)(`span`, {
                                            className: `mr-2`,
                                            children: `⚡`
                                        }),
                                        e.reasoning && (0, E.jsxs)(`details`, {
                                            className: `text-xs text-muted-foreground border-l-2 border-accent/50 pl-2 mb-2`,
                                            children: [
                                                (0, E.jsx)(`summary`, {
                                                    className: `cursor-pointer font-semibold select-none hover:text-white transition-colors`,
                                                    children: `Thought Process`
                                                }),
                                                (0, E.jsx)(`div`, {
                                                    className: `mt-1 font-mono whitespace-pre-wrap opacity-70`,
                                                    children: e.reasoning
                                                })
                                            ]
                                        }),
                                        e.tools && e.tools.length > 0 && (0, E.jsx)(`div`, {
                                            className: `flex flex-col gap-2 mb-2`,
                                            children: e.tools.map((e, t)=>(0, E.jsxs)(`details`, {
                                                    className: `bg-black/20 border border-white/5 rounded block`,
                                                    children: [
                                                        (0, E.jsxs)(`summary`, {
                                                            className: `text-xs cursor-pointer select-none py-1 px-2 text-purple-400 font-semibold hover:bg-white/5 transition-colors`,
                                                            children: [
                                                                `🔧 Tool: `,
                                                                e.name,
                                                                ` `,
                                                                e.success ? `✅` : `❌`
                                                            ]
                                                        }),
                                                        (0, E.jsxs)(`div`, {
                                                            className: `p-2 border-t border-white/5 text-[10px] font-mono`,
                                                            children: [
                                                                e.args && (0, E.jsxs)(`div`, {
                                                                    className: `mb-1`,
                                                                    children: [
                                                                        (0, E.jsx)(`strong`, {
                                                                            className: `text-muted-foreground`,
                                                                            children: `Input:`
                                                                        }),
                                                                        (0, E.jsx)(`pre`, {
                                                                            className: `whitespace-pre-wrap text-white/70 overflow-x-auto`,
                                                                            children: e.args
                                                                        })
                                                                    ]
                                                                }),
                                                                e.output && (0, E.jsxs)(`div`, {
                                                                    children: [
                                                                        (0, E.jsx)(`strong`, {
                                                                            className: `text-muted-foreground`,
                                                                            children: `Output:`
                                                                        }),
                                                                        (0, E.jsx)(`pre`, {
                                                                            className: `whitespace-pre-wrap text-white/70 overflow-x-auto max-h-40`,
                                                                            children: e.output
                                                                        })
                                                                    ]
                                                                })
                                                            ]
                                                        })
                                                    ]
                                                }, t))
                                        }),
                                        e.content && (0, E.jsx)(`div`, {
                                            className: `whitespace-pre-wrap`,
                                            children: e.content
                                        })
                                    ]
                                }, e.id))
                        }),
                        t && (i || r || a.length > 0) && (0, E.jsxs)(`div`, {
                            className: `max-w-[85%] p-4 rounded-xl text-sm leading-relaxed bg-white/5 border border-white/10 self-start flex flex-col gap-2`,
                            children: [
                                i && (0, E.jsxs)(`details`, {
                                    open: !0,
                                    className: `text-xs text-muted-foreground border-l-2 border-accent/50 pl-2 mb-1`,
                                    children: [
                                        (0, E.jsx)(`summary`, {
                                            className: `cursor-pointer font-semibold select-none hover:text-white transition-colors`,
                                            children: `Thinking Process`
                                        }),
                                        (0, E.jsx)(`div`, {
                                            className: `mt-1 font-mono whitespace-pre-wrap opacity-70`,
                                            children: i
                                        })
                                    ]
                                }),
                                a.length > 0 && (0, E.jsx)(`div`, {
                                    className: `flex flex-col gap-2 mb-1`,
                                    children: a.map((e, t)=>(0, E.jsxs)(`details`, {
                                            open: !0,
                                            className: `bg-black/20 border border-white/5 rounded block`,
                                            children: [
                                                (0, E.jsxs)(`summary`, {
                                                    className: `text-xs cursor-pointer select-none py-1 px-2 text-purple-400 font-semibold hover:bg-white/5 transition-colors`,
                                                    children: [
                                                        `🔧 Tool: `,
                                                        e.name,
                                                        ` `,
                                                        e.success ? `✅` : `❌`
                                                    ]
                                                }),
                                                (0, E.jsxs)(`div`, {
                                                    className: `p-2 border-t border-white/5 text-[10px] font-mono`,
                                                    children: [
                                                        e.args && (0, E.jsxs)(`div`, {
                                                            className: `mb-1`,
                                                            children: [
                                                                (0, E.jsx)(`strong`, {
                                                                    className: `text-muted-foreground`,
                                                                    children: `Input:`
                                                                }),
                                                                (0, E.jsx)(`pre`, {
                                                                    className: `whitespace-pre-wrap text-white/70 overflow-x-auto`,
                                                                    children: e.args
                                                                })
                                                            ]
                                                        }),
                                                        e.output && (0, E.jsxs)(`div`, {
                                                            children: [
                                                                (0, E.jsx)(`strong`, {
                                                                    className: `text-muted-foreground`,
                                                                    children: `Output:`
                                                                }),
                                                                (0, E.jsx)(`pre`, {
                                                                    className: `whitespace-pre-wrap text-white/70 overflow-x-auto max-h-40`,
                                                                    children: e.output
                                                                })
                                                            ]
                                                        })
                                                    ]
                                                })
                                            ]
                                        }, t))
                                }),
                                r && (0, E.jsx)(`div`, {
                                    className: `whitespace-pre-wrap`,
                                    children: r
                                })
                            ]
                        }),
                        s === `Compacting` && (0, E.jsxs)(`div`, {
                            className: `max-w-[85%] p-4 rounded-xl text-sm leading-relaxed bg-white/5 border border-white/10 self-start flex items-center gap-3`,
                            children: [
                                (0, E.jsx)(`span`, {
                                    className: `animate-spin text-base`,
                                    children: `🌪️`
                                }),
                                (0, E.jsx)(`span`, {
                                    className: `text-purple-400 font-mono font-bold animate-pulse`,
                                    children: `agent is compacting history. Please wait one moment.`
                                })
                            ]
                        })
                    ]
                }),
                (0, E.jsx)(`div`, {
                    className: `p-4 bg-black/20 border-t border-border/50`,
                    children: (0, E.jsxs)(`div`, {
                        className: `flex gap-3`,
                        children: [
                            (0, E.jsx)(`input`, {
                                value: c,
                                onChange: (e)=>l(e.target.value),
                                onKeyDown: (e)=>e.key === `Enter` && d(),
                                className: `flex-1 bg-white/5 border border-border/50 rounded-lg px-4 py-3 text-sm focus:outline-none focus:border-accent transition-colors shadow-inner text-white placeholder-muted-foreground`,
                                placeholder: `Enter objective...`,
                                disabled: t
                            }),
                            (0, E.jsx)(`button`, {
                                onClick: t ? ()=>{
                                    window.sendNexus && window.sendNexus(`StopStream`, {}), T.getState().commitStream();
                                } : d,
                                disabled: !t && !c,
                                className: `font-bold px-6 py-3 rounded-lg flex items-center justify-center transition-all shadow-lg hover:-translate-y-0.5 active:translate-y-0 ${t ? `bg-destructive hover:bg-destructive/90 text-white` : `bg-accent hover:bg-accent/90 text-background`}`,
                                children: t ? (0, E.jsx)(ge, {
                                    size: 18,
                                    className: `fill-current`
                                }) : (0, E.jsx)(Me, {
                                    size: 18
                                })
                            })
                        ]
                    })
                })
            ]
        });
    }
    var $e = [
        {
            id: `Thinking`,
            icon: S,
            label: `Thinking`
        },
        {
            id: `Planning`,
            icon: oe,
            label: `Planning`
        },
        {
            id: `Executing`,
            icon: Oe,
            label: `Executing`
        }
    ];
    function et(e) {
        let t = {
            tool_routing_stocks: `Rule: Stock Pricing`,
            tool_routing_http: `Rule: HTTP Fetching`,
            tool_routing_network: `Rule: Network Operations`,
            tool_routing_memory_search: `Rule: Memory Searches`,
            tool_routing_hallucination: `Rule: Tool Verification`,
            task_completion: `Rule: Task Completion`,
            tempest_identity: `Agent Persona`,
            code_quality_guideline: `Rule: Code Quality`,
            context_management: `Rule: Context Management`,
            file_modification_safety: `Rule: Change Safety`
        };
        if (t[e]) return t[e];
        if (e.toLowerCase().startsWith(`file `) && e.toLowerCase().includes(`indexed successfully`)) {
            let t = e.match(/`([^`]+)`/);
            if (t) return `Index: ${t[1]}`;
        }
        if (e.startsWith(`Skill: `) || e.startsWith(`Rust Project: `)) return e;
        if (e.length > 40) {
            let t = e.search(/[:,.]/);
            return t > 5 && t < 35 ? e.substring(0, t).trim() : e.substring(0, 37).trim() + `...`;
        }
        return e;
    }
    function tt({ mem: e }) {
        let [t, n] = (0, w.useState)(!1), [r, i] = (0, w.useState)(!1), a = (t)=>{
            t.stopPropagation(), navigator.clipboard.writeText(e.content), i(!0), setTimeout(()=>i(!1), 1500);
        }, o = et(e.topic), s = ((e)=>{
            let t = e;
            return t = t.replace(/^CORE INSTRUCTION \([^)]+\):\s*/i, ``), t = t.replace(/^CORE INSTRUCTION:\s*/i, ``), t.length > 95 ? t.substring(0, 92) + `...` : t;
        })(e.content);
        return (0, E.jsxs)(`div`, {
            className: `shrink-0 bg-white/[0.02] border border-white/5 rounded-xl overflow-hidden hover:bg-white/[0.04] transition-all duration-200 border-l-2 ${t ? `border-l-accent bg-white/[0.04] shadow-[0_0_15px_rgba(0,242,255,0.05)]` : `border-l-purple-500/30`}`,
            children: [
                (0, E.jsxs)(`div`, {
                    onClick: ()=>n(!t),
                    className: `p-3.5 flex items-start gap-3 cursor-pointer justify-between select-none min-h-[52px]`,
                    children: [
                        (0, E.jsxs)(`div`, {
                            className: `flex-1 min-w-0 flex flex-col gap-1.5`,
                            children: [
                                (0, E.jsxs)(`div`, {
                                    className: `flex items-center justify-between gap-2`,
                                    children: [
                                        (0, E.jsx)(`span`, {
                                            className: `font-bold text-[12px] text-white/95 tracking-wide font-mono truncate`,
                                            children: o
                                        }),
                                        (0, E.jsx)(`span`, {
                                            className: `text-[8px] text-muted-foreground font-mono shrink-0`,
                                            children: new Date(e.updated_at).toLocaleDateString()
                                        })
                                    ]
                                }),
                                !t && (0, E.jsx)(`p`, {
                                    className: `text-[10px] text-muted-foreground/85 leading-normal break-words font-sans`,
                                    children: s
                                }),
                                e.tags && (0, E.jsx)(`div`, {
                                    className: `flex flex-wrap gap-1 mt-0.5`,
                                    children: e.tags.split(`,`).map((e)=>(0, E.jsxs)(`span`, {
                                            className: `text-[8px] font-mono bg-purple-500/10 text-purple-400 border border-purple-500/20 px-1.5 py-0.5 rounded-full flex items-center gap-0.5`,
                                            children: [
                                                (0, E.jsx)(je, {
                                                    size: 8
                                                }),
                                                ` `,
                                                e.trim()
                                            ]
                                        }, e))
                                })
                            ]
                        }),
                        (0, E.jsx)(m.div, {
                            animate: {
                                rotate: t ? 180 : 0
                            },
                            transition: {
                                duration: .15
                            },
                            className: `text-muted-foreground mt-0.5 shrink-0`,
                            children: (0, E.jsx)(le, {
                                size: 14
                            })
                        })
                    ]
                }),
                (0, E.jsx)(`div`, {
                    className: `grid transition-all duration-200 ease-in-out ${t ? `grid-rows-[1fr] opacity-100` : `grid-rows-[0fr] opacity-0 pointer-events-none`}`,
                    children: (0, E.jsx)(`div`, {
                        className: `overflow-hidden`,
                        children: (0, E.jsxs)(`div`, {
                            className: `px-3.5 pb-3.5 border-t border-white/5 bg-black/25 flex flex-col gap-3 text-xs font-mono select-text`,
                            children: [
                                (0, E.jsx)(`div`, {
                                    className: `text-white/80 whitespace-pre-wrap leading-relaxed break-words py-2.5 px-3 bg-black/35 rounded-lg border border-white/5 text-[11px] mt-2`,
                                    children: e.content
                                }),
                                (0, E.jsxs)(`div`, {
                                    className: `flex items-center justify-between text-[8px] text-muted-foreground/60 select-none pt-1`,
                                    children: [
                                        (0, E.jsxs)(`span`, {
                                            className: `flex items-center gap-1 font-sans`,
                                            children: [
                                                (0, E.jsx)(b, {
                                                    size: 8
                                                }),
                                                ` Updated: `,
                                                new Date(e.updated_at).toLocaleString()
                                            ]
                                        }),
                                        (0, E.jsx)(`button`, {
                                            onClick: a,
                                            className: `flex items-center gap-1.5 hover:text-white transition-all py-1 px-2 rounded bg-white/5 border border-white/5 hover:border-white/10 cursor-pointer font-bold font-sans`,
                                            title: `Copy memory content`,
                                            children: r ? (0, E.jsxs)(E.Fragment, {
                                                children: [
                                                    (0, E.jsx)(de, {
                                                        size: 8,
                                                        className: `text-green-400`
                                                    }),
                                                    (0, E.jsx)(`span`, {
                                                        className: `text-green-400 font-sans`,
                                                        children: `COPIED`
                                                    })
                                                ]
                                            }) : (0, E.jsxs)(E.Fragment, {
                                                children: [
                                                    (0, E.jsx)(ee, {
                                                        size: 8
                                                    }),
                                                    (0, E.jsx)(`span`, {
                                                        className: `font-sans`,
                                                        children: `COPY`
                                                    })
                                                ]
                                            })
                                        })
                                    ]
                                })
                            ]
                        })
                    })
                })
            ]
        });
    }
    function nt() {
        let { agentPhase: e, currentTask: t, activeTools: n, memories: r } = T(), [i, a] = (0, w.useState)(``), o = (t)=>t === `Planning` && e === `PendingTools` || t === `Executing` && e === `ExecutingTools` ? !0 : e === t, s = ()=>{
            window.sendNexus && window.sendNexus(`GetMemories`, {});
        }, c = r.filter((e)=>e.topic.toLowerCase().includes(i.toLowerCase()) || e.content.toLowerCase().includes(i.toLowerCase()) || e.tags && e.tags.toLowerCase().includes(i.toLowerCase()));
        return (0, E.jsxs)(`div`, {
            className: `flex flex-col gap-5 select-text h-full max-h-full overflow-hidden`,
            children: [
                (0, E.jsxs)(`div`, {
                    className: `flex justify-between items-center px-2 relative flex-none`,
                    children: [
                        (0, E.jsx)(`div`, {
                            className: `absolute top-5 left-8 right-8 h-0.5 bg-border -z-10`
                        }),
                        $e.map((e)=>{
                            let t = o(e.id);
                            return (0, E.jsxs)(`div`, {
                                className: `flex flex-col items-center gap-2 bg-transparent`,
                                children: [
                                    (0, E.jsx)(m.div, {
                                        animate: {
                                            scale: t ? 1.15 : 1,
                                            boxShadow: t ? `0 0 15px rgba(0,242,255,0.4)` : `none`
                                        },
                                        className: `w-10 h-10 rounded-full flex items-center justify-center transition-colors ${t ? `bg-accent text-background` : `glass-panel border border-border text-muted-foreground`}`,
                                        children: (0, E.jsx)(e.icon, {
                                            size: 18
                                        })
                                    }),
                                    (0, E.jsx)(`span`, {
                                        className: `text-[10px] uppercase font-bold tracking-wider ${t ? `text-accent` : `text-muted-foreground`}`,
                                        children: e.label
                                    })
                                ]
                            }, e.id);
                        })
                    ]
                }),
                t !== `--` && (0, E.jsxs)(`div`, {
                    className: `bg-white/5 border border-white/10 p-3 rounded-lg flex-none`,
                    children: [
                        (0, E.jsx)(`h4`, {
                            className: `text-[10px] uppercase font-bold tracking-wider text-muted-foreground mb-1`,
                            children: `Current Task`
                        }),
                        (0, E.jsx)(`p`, {
                            className: `text-xs font-mono text-white leading-normal`,
                            children: t
                        })
                    ]
                }),
                (0, E.jsxs)(`div`, {
                    className: `flex-none`,
                    children: [
                        (0, E.jsx)(`h4`, {
                            className: `text-[10px] uppercase font-bold tracking-wider text-muted-foreground mb-2`,
                            children: `Active Tools`
                        }),
                        (0, E.jsx)(`div`, {
                            className: `flex flex-col gap-2`,
                            children: n.length === 0 ? (0, E.jsx)(`span`, {
                                className: `text-xs italic text-muted-foreground border border-transparent p-1`,
                                children: `No tools running`
                            }) : n.map((e, t)=>(0, E.jsxs)(m.div, {
                                    initial: {
                                        opacity: 0,
                                        x: -10
                                    },
                                    animate: {
                                        opacity: 1,
                                        x: 0
                                    },
                                    className: `bg-accent/10 border border-accent/30 px-3 py-2 rounded-md flex items-center gap-2 text-xs font-mono text-accent`,
                                    children: [
                                        (0, E.jsx)(`span`, {
                                            className: `animate-spin text-[10px]`,
                                            children: `⚙️`
                                        }),
                                        ` `,
                                        e
                                    ]
                                }, t))
                        })
                    ]
                }),
                (0, E.jsx)(`div`, {
                    className: `border-t border-border/40 flex-none`
                }),
                (0, E.jsxs)(`div`, {
                    className: `flex-1 min-h-0 flex flex-col gap-3 overflow-hidden`,
                    children: [
                        (0, E.jsxs)(`div`, {
                            className: `flex items-center justify-between flex-none`,
                            children: [
                                (0, E.jsxs)(`div`, {
                                    className: `flex items-center gap-2`,
                                    children: [
                                        (0, E.jsx)(S, {
                                            size: 14,
                                            className: `text-accent`
                                        }),
                                        (0, E.jsx)(`h4`, {
                                            className: `text-[10px] uppercase font-bold tracking-wider text-white`,
                                            children: `THE BRAIN`
                                        })
                                    ]
                                }),
                                (0, E.jsx)(`button`, {
                                    onClick: s,
                                    className: `p-1 hover:bg-white/5 border border-transparent hover:border-white/10 rounded transition-all cursor-pointer text-muted-foreground hover:text-white`,
                                    title: `Refresh memory store`,
                                    children: (0, E.jsx)(Pe, {
                                        size: 12
                                    })
                                })
                            ]
                        }),
                        (0, E.jsxs)(`div`, {
                            className: `relative flex-none`,
                            children: [
                                (0, E.jsx)(Fe, {
                                    size: 12,
                                    className: `absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground`
                                }),
                                (0, E.jsx)(`input`, {
                                    value: i,
                                    onChange: (e)=>a(e.target.value),
                                    placeholder: `Search agent memories...`,
                                    className: `w-full bg-white/5 border border-border/50 rounded-lg pl-8 pr-4 py-2 text-xs focus:outline-none focus:border-accent transition-colors text-white placeholder-muted-foreground`
                                })
                            ]
                        }),
                        (0, E.jsx)(`div`, {
                            className: `flex-1 overflow-y-auto pr-1 flex flex-col gap-2 min-h-0 scroll-smooth`,
                            children: c.length === 0 ? (0, E.jsx)(`div`, {
                                className: `text-center py-6 text-xs text-muted-foreground italic border border-white/5 rounded-xl bg-white/[0.01]`,
                                children: `No memories found matching filter.`
                            }) : c.map((e)=>(0, E.jsx)(tt, {
                                    mem: e
                                }, e.topic))
                        })
                    ]
                })
            ]
        });
    }
    Re.config({
        paths: {
            vs: `/monaco-editor/min/vs`
        }
    });
    function rt() {
        let { activeFile: e, setEditorFocused: t, isFileEditable: n } = T();
        return e ? (0, E.jsx)(`div`, {
            className: `flex-1 w-full h-full`,
            onFocus: ()=>t(!0),
            onBlur: ()=>t(!1),
            children: (0, E.jsx)(ze, {
                height: `100%`,
                language: {
                    rs: `rust`,
                    zig: `zig`,
                    ts: `typescript`,
                    tsx: `typescript`,
                    js: `javascript`,
                    jsx: `javascript`,
                    sh: `shell`,
                    bash: `shell`,
                    zsh: `shell`,
                    fish: `shell`,
                    nix: `nix`,
                    toml: `toml`,
                    lock: `toml`,
                    md: `markdown`,
                    markdown: `markdown`,
                    json: `json`,
                    html: `html`,
                    css: `css`,
                    py: `python`,
                    yml: `yaml`,
                    yaml: `yaml`,
                    c: `c`,
                    cpp: `cpp`,
                    h: `cpp`,
                    cmake: `cmake`,
                    sshconfig: `ini`,
                    "ssh-config": `ini`,
                    ssh: `ini`,
                    asm: `assembly`,
                    s: `assembly`,
                    txt: `plaintext`
                }[e.ext.toLowerCase()] || `plaintext`,
                theme: `vs-dark`,
                value: e.content,
                onChange: (e)=>T.getState().updateActiveFileContent(e || ``),
                beforeMount: (e)=>{
                    e.languages.getLanguages().some((e)=>e.id === `toml`) || (e.languages.register({
                        id: `toml`
                    }), e.languages.setMonarchTokensProvider(`toml`, {
                        defaultToken: ``,
                        tokenPostfix: `.toml`,
                        keywords: [
                            `true`,
                            `false`
                        ],
                        tokenizer: {
                            root: [
                                [
                                    /\[[^\]]+\]/,
                                    `metatag`
                                ],
                                [
                                    /[a-zA-Z0-9_-]+(?=\s*=)/,
                                    `attribute.name`
                                ],
                                [
                                    /(=)/,
                                    `operator`
                                ],
                                [
                                    /[a-zA-Z_]\w*/,
                                    {
                                        cases: {
                                            "@keywords": `keyword`,
                                            "@default": `identifier`
                                        }
                                    }
                                ],
                                [
                                    /#.*$/,
                                    `comment`
                                ],
                                [
                                    /\d+(\.\d+)?/,
                                    `number`
                                ],
                                [
                                    /"([^"\\]|\\.)*"/,
                                    `string`
                                ],
                                [
                                    /'([^'\\]|\\.)*'/,
                                    `string`
                                ]
                            ]
                        }
                    }));
                },
                options: {
                    readOnly: !n,
                    minimap: {
                        enabled: !1
                    },
                    fontSize: 13,
                    fontFamily: `"JetBrains Mono", monospace`,
                    scrollBeyondLastLine: !1,
                    smoothScrolling: !0,
                    cursorBlinking: `smooth`
                },
                loading: (0, E.jsx)(`div`, {
                    className: `text-accent animate-pulse font-mono text-sm p-4`,
                    children: `Loading editor...`
                })
            })
        }) : (0, E.jsx)(`div`, {
            className: `flex-1 flex items-center justify-center bg-black/40 text-muted-foreground text-sm font-mono p-4`,
            children: (0, E.jsxs)(`div`, {
                className: `text-center`,
                children: [
                    (0, E.jsx)(`p`, {
                        className: `mb-2`,
                        children: `No active file`
                    }),
                    (0, E.jsx)(`p`, {
                        className: `opacity-50`,
                        children: `Select a file from the explorer to view it here.`
                    })
                ]
            })
        });
    }
    function it() {
        let [e, t] = (0, w.useState)(!1), { setBackgroundIntensity: n, backgroundIntensity: r } = T();
        return (0, w.useEffect)(()=>{
            let e = (e)=>{
                e.key === `k` && (e.metaKey || e.ctrlKey) && (e.preventDefault(), t((e)=>!e));
            };
            return document.addEventListener(`keydown`, e), ()=>document.removeEventListener(`keydown`, e);
        }, []), e ? (0, E.jsx)(`div`, {
            className: `fixed inset-0 z-[100] flex items-start justify-center pt-[15vh] bg-black/40 backdrop-blur-sm`,
            onClick: ()=>t(!1),
            children: (0, E.jsx)(`div`, {
                className: `w-[600px] max-w-full glass-panel border border-border/70 rounded-xl shadow-[0_0_40px_rgba(0,0,0,0.6)] overflow-hidden`,
                onClick: (e)=>e.stopPropagation(),
                children: (0, E.jsxs)(l, {
                    label: `Global Command Menu`,
                    className: `w-full text-foreground bg-transparent`,
                    shouldFilter: !0,
                    children: [
                        (0, E.jsx)(l.Input, {
                            autoFocus: !0,
                            className: `w-full bg-transparent px-4 py-4 border-b border-border/50 focus:outline-none placeholder:text-muted-foreground text-[15px]`,
                            placeholder: `Type a command or search... (e.g. 'background')`
                        }),
                        (0, E.jsxs)(l.List, {
                            className: `p-2 max-h-[400px] overflow-y-auto`,
                            children: [
                                (0, E.jsx)(l.Empty, {
                                    className: `p-6 text-center text-muted-foreground text-sm`,
                                    children: `No results found.`
                                }),
                                (0, E.jsxs)(l.Group, {
                                    heading: `Settings: Appearance`,
                                    className: `text-xs font-semibold text-muted-foreground uppercase tracking-wider mb-1 px-3 pt-3`,
                                    children: [
                                        (0, E.jsxs)(l.Item, {
                                            onSelect: ()=>{
                                                n(`subtle`), t(!1);
                                            },
                                            className: `flex items-center px-3 py-3 mt-1 rounded-lg cursor-pointer hover:bg-accent/20 aria-selected:bg-accent/20 text-foreground text-sm transition-colors`,
                                            children: [
                                                `Set Background Intensity: Subtle`,
                                                ` `,
                                                r === `subtle` && (0, E.jsx)(`span`, {
                                                    className: `ml-2 text-accent`,
                                                    children: `✓`
                                                })
                                            ]
                                        }),
                                        (0, E.jsxs)(l.Item, {
                                            onSelect: ()=>{
                                                n(`medium`), t(!1);
                                            },
                                            className: `flex items-center px-3 py-3 mt-1 rounded-lg cursor-pointer hover:bg-accent/20 aria-selected:bg-accent/20 text-foreground text-sm transition-colors`,
                                            children: [
                                                `Set Background Intensity: Medium`,
                                                ` `,
                                                r === `medium` && (0, E.jsx)(`span`, {
                                                    className: `ml-2 text-accent`,
                                                    children: `✓`
                                                })
                                            ]
                                        }),
                                        (0, E.jsxs)(l.Item, {
                                            onSelect: ()=>{
                                                n(`full`), t(!1);
                                            },
                                            className: `flex items-center px-3 py-3 mt-1 rounded-lg cursor-pointer hover:bg-accent/20 aria-selected:bg-accent/20 text-foreground text-sm transition-colors`,
                                            children: [
                                                `Set Background Intensity: Full`,
                                                ` `,
                                                r === `full` && (0, E.jsx)(`span`, {
                                                    className: `ml-2 text-accent`,
                                                    children: `✓`
                                                })
                                            ]
                                        })
                                    ]
                                }),
                                (0, E.jsxs)(l.Group, {
                                    heading: `Workspace`,
                                    className: `text-xs font-semibold text-muted-foreground uppercase tracking-wider mb-1 px-3 pt-4`,
                                    children: [
                                        (0, E.jsx)(l.Item, {
                                            onSelect: ()=>{
                                                T.getState().setTerminalOpen(!T.getState().isTerminalOpen), t(!1);
                                            },
                                            className: `flex items-center px-3 py-3 mt-1 rounded-lg cursor-pointer hover:bg-accent/20 aria-selected:bg-accent/20 text-foreground text-sm transition-colors`,
                                            children: `Toggle Terminal Panel`
                                        }),
                                        (0, E.jsx)(l.Item, {
                                            onSelect: ()=>{
                                                window.sendNexus && window.sendNexus(`ClearChat`, {}), t(!1);
                                            },
                                            className: `flex items-center px-3 py-3 mt-1 rounded-lg cursor-pointer hover:bg-accent/20 aria-selected:bg-accent/20 text-foreground text-sm transition-colors`,
                                            children: `Clear Chat History`
                                        })
                                    ]
                                })
                            ]
                        })
                    ]
                })
            })
        }) : null;
    }
    function at() {
        let [e, t] = (0, w.useState)(``), [n, r] = (0, w.useState)(!0), { searchResults: i, isSearching: a, setSearching: o, setSearchResults: s, engineStatus: c, plannerModel: l, executorModel: u, verifierModel: d, kvCacheHitPct: f, planningDurationMs: h, executingDurationMs: g, verifyingDurationMs: _ } = T(), v = ()=>{
            !e.trim() || a || (o(!0), s([]), window.sendNexus && window.sendNexus(`SearchFiles`, {
                query: e,
                path: `.`
            }));
        }, y = (e)=>{
            window.sendNexus && window.sendNexus(`ReadFile`, {
                path: e
            });
        };
        return (0, E.jsxs)(`div`, {
            className: `flex flex-col h-full gap-4`,
            children: [
                (0, E.jsxs)(`div`, {
                    className: `bg-white/[0.01] border border-white/5 rounded-xl flex flex-col flex-none hover:border-white/10 transition-colors overflow-hidden`,
                    children: [
                        (0, E.jsxs)(`div`, {
                            onClick: ()=>r(!n),
                            className: `p-3 flex items-center justify-between cursor-pointer select-none`,
                            children: [
                                (0, E.jsxs)(`h4`, {
                                    className: `text-[10px] uppercase font-bold tracking-wider text-muted-foreground flex items-center gap-1.5`,
                                    children: [
                                        (0, E.jsx)(ae, {
                                            size: 12,
                                            className: `text-accent`
                                        }),
                                        ` Active Models & Engine`
                                    ]
                                }),
                                (0, E.jsx)(m.div, {
                                    animate: {
                                        rotate: n ? 180 : 0
                                    },
                                    transition: {
                                        duration: .15
                                    },
                                    className: `text-muted-foreground`,
                                    children: (0, E.jsx)(le, {
                                        size: 14
                                    })
                                })
                            ]
                        }),
                        (0, E.jsx)(`div`, {
                            className: `grid transition-all duration-200 ease-in-out ${n ? `grid-rows-[1fr] opacity-100` : `grid-rows-[0fr] opacity-0`}`,
                            children: (0, E.jsx)(`div`, {
                                className: `overflow-hidden`,
                                children: (0, E.jsxs)(`div`, {
                                    className: `px-3 pb-3`,
                                    children: [
                                        l === u && u === d && l !== `--` ? (0, E.jsxs)(`div`, {
                                            className: `flex flex-col gap-2 text-[10px] font-mono`,
                                            children: [
                                                (0, E.jsxs)(`div`, {
                                                    className: `bg-black/35 border border-white/5 px-3 py-2.5 rounded-lg flex flex-col gap-1.5 select-text hover:bg-black/45 transition-colors`,
                                                    children: [
                                                        (0, E.jsxs)(`div`, {
                                                            className: `flex items-center justify-between`,
                                                            children: [
                                                                (0, E.jsx)(`span`, {
                                                                    className: `text-muted-foreground/60 uppercase text-[8px] tracking-wider font-semibold`,
                                                                    children: `Engine`
                                                                }),
                                                                (0, E.jsx)(`span`, {
                                                                    className: `text-[7px] font-bold uppercase tracking-widest bg-accent/15 text-accent border border-accent/25 px-2 py-0.5 rounded-full`,
                                                                    children: `VRAM Sharing`
                                                                })
                                                            ]
                                                        }),
                                                        (0, E.jsx)(`span`, {
                                                            className: `text-accent truncate font-bold text-[11px]`,
                                                            title: c,
                                                            children: c.replace(` (VRAM Sharing)`, ``)
                                                        })
                                                    ]
                                                }),
                                                (0, E.jsxs)(`div`, {
                                                    className: `bg-black/35 border border-white/5 px-3 py-2.5 rounded-lg flex flex-col gap-1.5 select-text hover:bg-black/45 transition-colors`,
                                                    children: [
                                                        (0, E.jsxs)(`span`, {
                                                            className: `text-muted-foreground/60 uppercase text-[8px] tracking-wider font-semibold flex items-center gap-1`,
                                                            children: [
                                                                (0, E.jsx)(S, {
                                                                    size: 8,
                                                                    className: `text-purple-400`
                                                                }),
                                                                ` Unified Model`
                                                            ]
                                                        }),
                                                        (0, E.jsx)(`span`, {
                                                            className: `text-purple-400 truncate font-bold text-[11px]`,
                                                            title: l,
                                                            children: l
                                                        }),
                                                        (0, E.jsx)(`span`, {
                                                            className: `text-[8px] text-muted-foreground/50 leading-tight`,
                                                            children: `Planner · Executor · Verifier — single model, dynamic system prompt switching`
                                                        })
                                                    ]
                                                })
                                            ]
                                        }) : (0, E.jsxs)(`div`, {
                                            className: `grid grid-cols-2 gap-2 text-[10px] font-mono`,
                                            children: [
                                                (0, E.jsxs)(`div`, {
                                                    className: `bg-black/35 border border-white/5 px-2.5 py-1.5 rounded-lg flex flex-col gap-0.5 select-text hover:bg-black/45 transition-colors`,
                                                    children: [
                                                        (0, E.jsx)(`span`, {
                                                            className: `text-muted-foreground/60 uppercase text-[8px] tracking-wider font-semibold`,
                                                            children: `Engine`
                                                        }),
                                                        (0, E.jsx)(`span`, {
                                                            className: `text-accent truncate font-bold`,
                                                            title: c,
                                                            children: c
                                                        })
                                                    ]
                                                }),
                                                (0, E.jsxs)(`div`, {
                                                    className: `bg-black/35 border border-white/5 px-2.5 py-1.5 rounded-lg flex flex-col gap-0.5 select-text hover:bg-black/45 transition-colors`,
                                                    children: [
                                                        (0, E.jsxs)(`span`, {
                                                            className: `text-muted-foreground/60 uppercase text-[8px] tracking-wider font-semibold flex items-center gap-1`,
                                                            children: [
                                                                (0, E.jsx)(S, {
                                                                    size: 8,
                                                                    className: `text-purple-400`
                                                                }),
                                                                ` Planner`
                                                            ]
                                                        }),
                                                        (0, E.jsx)(`span`, {
                                                            className: `text-purple-400 truncate font-bold`,
                                                            title: l,
                                                            children: l
                                                        })
                                                    ]
                                                }),
                                                (0, E.jsxs)(`div`, {
                                                    className: `bg-black/35 border border-white/5 px-2.5 py-1.5 rounded-lg flex flex-col gap-0.5 select-text hover:bg-black/45 transition-colors`,
                                                    children: [
                                                        (0, E.jsxs)(`span`, {
                                                            className: `text-muted-foreground/60 uppercase text-[8px] tracking-wider font-semibold flex items-center gap-1`,
                                                            children: [
                                                                (0, E.jsx)(Oe, {
                                                                    size: 8,
                                                                    className: `text-pink-400`
                                                                }),
                                                                ` Executor`
                                                            ]
                                                        }),
                                                        (0, E.jsx)(`span`, {
                                                            className: `text-pink-400 truncate font-bold`,
                                                            title: u,
                                                            children: u
                                                        })
                                                    ]
                                                }),
                                                (0, E.jsxs)(`div`, {
                                                    className: `bg-black/35 border border-white/5 px-2.5 py-1.5 rounded-lg flex flex-col gap-0.5 select-text hover:bg-black/45 transition-colors`,
                                                    children: [
                                                        (0, E.jsxs)(`span`, {
                                                            className: `text-muted-foreground/60 uppercase text-[8px] tracking-wider font-semibold flex items-center gap-1`,
                                                            children: [
                                                                (0, E.jsx)(ye, {
                                                                    size: 8,
                                                                    className: `text-emerald-400`
                                                                }),
                                                                ` Verifier`
                                                            ]
                                                        }),
                                                        (0, E.jsx)(`span`, {
                                                            className: `text-emerald-400 truncate font-bold`,
                                                            title: d,
                                                            children: d
                                                        })
                                                    ]
                                                })
                                            ]
                                        }),
                                        f !== null && (0, E.jsxs)(`div`, {
                                            className: `mt-3 pt-3 border-t border-white/5 flex flex-col gap-2 font-mono text-[10px]`,
                                            children: [
                                                (0, E.jsxs)(`div`, {
                                                    className: `flex items-center justify-between`,
                                                    children: [
                                                        (0, E.jsxs)(`span`, {
                                                            className: `text-muted-foreground/60 uppercase text-[8px] tracking-wider font-semibold flex items-center gap-1.5`,
                                                            children: [
                                                                (0, E.jsx)(re, {
                                                                    size: 10,
                                                                    className: `text-[#00f2ff]`
                                                                }),
                                                                ` KV Cache Hit Rate`
                                                            ]
                                                        }),
                                                        (0, E.jsxs)(`span`, {
                                                            className: `text-[#00f2ff] font-bold font-mono`,
                                                            children: [
                                                                f.toFixed(1),
                                                                `%`
                                                            ]
                                                        })
                                                    ]
                                                }),
                                                (0, E.jsx)(`div`, {
                                                    className: `w-full bg-black/40 border border-white/5 h-2 rounded-full overflow-hidden relative`,
                                                    children: (0, E.jsx)(m.div, {
                                                        initial: {
                                                            width: 0
                                                        },
                                                        animate: {
                                                            width: `${f}%`
                                                        },
                                                        transition: {
                                                            duration: .8,
                                                            ease: `easeOut`
                                                        },
                                                        className: `bg-gradient-to-r from-purple-500 to-[#00f2ff] h-full rounded-full`
                                                    })
                                                })
                                            ]
                                        }),
                                        (h !== null || g !== null || _ !== null) && (0, E.jsxs)(`div`, {
                                            className: `mt-3 pt-3 border-t border-white/5 flex flex-col gap-2 font-mono text-[10px]`,
                                            children: [
                                                (0, E.jsxs)(`span`, {
                                                    className: `text-muted-foreground/60 uppercase text-[8px] tracking-wider font-semibold flex items-center gap-1.5 mb-1`,
                                                    children: [
                                                        (0, E.jsx)(C, {
                                                            size: 10,
                                                            className: `text-cyan-400`
                                                        }),
                                                        ` Turn Phase Durations`
                                                    ]
                                                }),
                                                h !== null && (0, E.jsxs)(`div`, {
                                                    className: `flex flex-col gap-1`,
                                                    children: [
                                                        (0, E.jsxs)(`div`, {
                                                            className: `flex items-center justify-between text-[9px]`,
                                                            children: [
                                                                (0, E.jsx)(`span`, {
                                                                    className: `text-purple-400 font-semibold`,
                                                                    children: `Planning`
                                                                }),
                                                                (0, E.jsxs)(`span`, {
                                                                    className: `text-purple-300 font-bold`,
                                                                    children: [
                                                                        h,
                                                                        ` ms`
                                                                    ]
                                                                })
                                                            ]
                                                        }),
                                                        (0, E.jsx)(`div`, {
                                                            className: `w-full bg-black/40 h-1.5 rounded-full overflow-hidden relative border border-white/5`,
                                                            children: (0, E.jsx)(m.div, {
                                                                initial: {
                                                                    width: 0
                                                                },
                                                                animate: {
                                                                    width: `${Math.min(100, h / 1e4 * 100)}%`
                                                                },
                                                                transition: {
                                                                    duration: .8,
                                                                    ease: `easeOut`
                                                                },
                                                                className: `bg-purple-500 h-full rounded-full`
                                                            })
                                                        })
                                                    ]
                                                }),
                                                g !== null && (0, E.jsxs)(`div`, {
                                                    className: `flex flex-col gap-1`,
                                                    children: [
                                                        (0, E.jsxs)(`div`, {
                                                            className: `flex items-center justify-between text-[9px]`,
                                                            children: [
                                                                (0, E.jsx)(`span`, {
                                                                    className: `text-pink-400 font-semibold`,
                                                                    children: `Execution (Tool Use)`
                                                                }),
                                                                (0, E.jsxs)(`span`, {
                                                                    className: `text-pink-300 font-bold`,
                                                                    children: [
                                                                        g,
                                                                        ` ms`
                                                                    ]
                                                                })
                                                            ]
                                                        }),
                                                        (0, E.jsx)(`div`, {
                                                            className: `w-full bg-black/40 h-1.5 rounded-full overflow-hidden relative border border-white/5`,
                                                            children: (0, E.jsx)(m.div, {
                                                                initial: {
                                                                    width: 0
                                                                },
                                                                animate: {
                                                                    width: `${Math.min(100, g / 15e3 * 100)}%`
                                                                },
                                                                transition: {
                                                                    duration: .8,
                                                                    ease: `easeOut`
                                                                },
                                                                className: `bg-pink-500 h-full rounded-full`
                                                            })
                                                        })
                                                    ]
                                                }),
                                                _ !== null && (0, E.jsxs)(`div`, {
                                                    className: `flex flex-col gap-1`,
                                                    children: [
                                                        (0, E.jsxs)(`div`, {
                                                            className: `flex items-center justify-between text-[9px]`,
                                                            children: [
                                                                (0, E.jsx)(`span`, {
                                                                    className: `text-emerald-400 font-semibold`,
                                                                    children: `Verification`
                                                                }),
                                                                (0, E.jsxs)(`span`, {
                                                                    className: `text-emerald-300 font-bold`,
                                                                    children: [
                                                                        _,
                                                                        ` ms`
                                                                    ]
                                                                })
                                                            ]
                                                        }),
                                                        (0, E.jsx)(`div`, {
                                                            className: `w-full bg-black/40 h-1.5 rounded-full overflow-hidden relative border border-white/5`,
                                                            children: (0, E.jsx)(m.div, {
                                                                initial: {
                                                                    width: 0
                                                                },
                                                                animate: {
                                                                    width: `${Math.min(100, _ / 1e4 * 100)}%`
                                                                },
                                                                transition: {
                                                                    duration: .8,
                                                                    ease: `easeOut`
                                                                },
                                                                className: `bg-emerald-500 h-full rounded-full`
                                                            })
                                                        })
                                                    ]
                                                })
                                            ]
                                        })
                                    ]
                                })
                            })
                        })
                    ]
                }),
                (0, E.jsxs)(`div`, {
                    className: `bg-white/[0.01] border border-white/5 rounded-xl flex flex-col flex-none hover:border-white/10 transition-colors p-3 mb-1`,
                    children: [
                        (0, E.jsxs)(`h4`, {
                            className: `text-[10px] uppercase font-bold tracking-wider text-muted-foreground flex items-center gap-1.5 mb-2`,
                            children: [
                                (0, E.jsx)(S, {
                                    size: 12,
                                    className: `text-accent`
                                }),
                                ` Quick Actions`
                            ]
                        }),
                        (0, E.jsx)(`div`, {
                            className: `grid grid-cols-2 gap-2`,
                            children: [
                                {
                                    label: `Generate tests for current file`,
                                    query: `Generate tests for the current file.`
                                },
                                {
                                    label: `Optimize function speed`,
                                    query: `Optimize the current function or file for maximum performance and speed.`
                                },
                                {
                                    label: `Add error handling`,
                                    query: `Add robust error handling everywhere in the current file.`
                                },
                                {
                                    label: `Explain like I'm 10`,
                                    query: `Explain this codebase to me like I am 10 years old.`
                                },
                                {
                                    label: `Audit for security issues`,
                                    query: `Audit the current file for potential security vulnerabilities and edge cases.`
                                },
                                {
                                    label: `Refactor for readability`,
                                    query: `Refactor the code to improve readability, variable naming, and maintainability.`
                                },
                                {
                                    label: `Generate documentation`,
                                    query: `Generate comprehensive docstrings and comments for all public functions and types.`
                                },
                                {
                                    label: `Find hidden bugs & leaks`,
                                    query: `Analyze the current file for memory leaks, resource exhaustion, and subtle hidden bugs.`
                                }
                            ].map((e, t)=>(0, E.jsx)(`button`, {
                                    onClick: ()=>{
                                        j(), window.sendNexus && window.sendNexus(`Chat`, {
                                            message: e.query
                                        });
                                    },
                                    className: `text-[10px] font-semibold text-left p-2 rounded bg-white/5 hover:bg-accent/20 hover:text-accent border border-white/5 hover:border-accent/30 transition-all text-muted-foreground cursor-pointer`,
                                    children: e.label
                                }, t))
                        })
                    ]
                }),
                (0, E.jsxs)(`div`, {
                    className: `flex gap-2 p-2 mb-3 bg-white/5 rounded-lg border border-white/10`,
                    children: [
                        (0, E.jsx)(`input`, {
                            type: `text`,
                            value: e,
                            onChange: (e)=>t(e.target.value),
                            onKeyDown: (e)=>e.key === `Enter` && v(),
                            className: `flex-1 bg-transparent px-2 py-1 text-sm focus:outline-none text-white placeholder-muted-foreground`,
                            placeholder: `Search regex / string...`,
                            disabled: a
                        }),
                        (0, E.jsx)(`button`, {
                            onClick: v,
                            disabled: a || !e.trim(),
                            className: `p-2 bg-accent text-background rounded hover:bg-accent/90 transition-colors disabled:opacity-50 flex items-center justify-center`,
                            children: a ? (0, E.jsx)(Ne, {
                                size: 14,
                                className: `animate-spin`
                            }) : (0, E.jsx)(Fe, {
                                size: 14
                            })
                        })
                    ]
                }),
                (0, E.jsxs)(`div`, {
                    className: `flex-1 overflow-y-auto`,
                    children: [
                        a && (0, E.jsxs)(`div`, {
                            className: `flex flex-col items-center justify-center p-6 text-muted-foreground text-sm font-mono gap-2`,
                            children: [
                                (0, E.jsx)(Ne, {
                                    className: `animate-spin text-accent`,
                                    size: 20
                                }),
                                (0, E.jsx)(`span`, {
                                    children: `Scanning project tree...`
                                })
                            ]
                        }),
                        !a && i.length === 0 && e && (0, E.jsx)(`p`, {
                            className: `text-muted-foreground text-sm p-4 text-center italic`,
                            children: `No matches found`
                        }),
                        (0, E.jsx)(`div`, {
                            className: `flex flex-col gap-3`,
                            children: (0, E.jsx)(p, {
                                children: !a && i.map((e, t)=>(0, E.jsxs)(m.div, {
                                        onClick: ()=>y(e.file),
                                        initial: {
                                            opacity: 0,
                                            y: 5
                                        },
                                        animate: {
                                            opacity: 1,
                                            y: 0
                                        },
                                        transition: {
                                            delay: Math.min(t * .02, .3)
                                        },
                                        className: `p-3 bg-white/5 border border-white/5 hover:border-accent/40 rounded-lg cursor-pointer transition-all hover:bg-accent/5`,
                                        children: [
                                            (0, E.jsxs)(`div`, {
                                                className: `flex items-center gap-2 mb-1 text-xs font-mono text-accent truncate`,
                                                children: [
                                                    (0, E.jsx)(xe, {
                                                        size: 12
                                                    }),
                                                    (0, E.jsx)(`span`, {
                                                        className: `truncate`,
                                                        children: e.file
                                                    }),
                                                    (0, E.jsxs)(`span`, {
                                                        className: `text-muted-foreground ml-auto`,
                                                        children: [
                                                            `Line `,
                                                            e.line
                                                        ]
                                                    })
                                                ]
                                            }),
                                            (0, E.jsx)(`pre`, {
                                                className: `text-xs font-mono text-muted-foreground truncate bg-black/30 p-1.5 rounded border border-white/5`,
                                                children: e.content
                                            })
                                        ]
                                    }, `${e.file}-${e.line}-${t}`))
                            })
                        })
                    ]
                })
            ]
        });
    }
    function ot() {
        let { backgroundIntensity: e, setBackgroundIntensity: t, sliderAggressiveCareful: n, sliderCreativePrecise: r, sliderFastThorough: i, activeRole: a, contextLimit: o, muteSounds: s, setSliderAggressiveCareful: c, setSliderCreativePrecise: l, setSliderFastThorough: u, setActiveRole: d, setContextLimit: f, setMuteSounds: p } = T(), m = [
            {
                id: `subtle`,
                label: `Subtle`
            },
            {
                id: `medium`,
                label: `Medium`
            },
            {
                id: `full`,
                label: `Full`
            }
        ], h = [
            {
                id: `pair-programmer`,
                label: `Pair Programmer`,
                desc: `Collaborative development assistant. Focused on step-by-step logic, clear designs, and code-review cooperation.`,
                icon: (0, E.jsx)(Ce, {
                    size: 14,
                    className: `text-blue-400`
                })
            },
            {
                id: `senior-editor`,
                label: `Senior Editor`,
                desc: `Focused on codebase styling, structure, clean architecture, readability, and ensuring code quality rules are followed.`,
                icon: (0, E.jsx)(we, {
                    size: 14,
                    className: `text-yellow-400`
                })
            },
            {
                id: `security-auditor`,
                label: `Security Auditor`,
                desc: `Focused on vetting inputs, checks sanitization, and hunting for injection, race conditions, or execution vulnerability vectors.`,
                icon: (0, E.jsx)(ve, {
                    size: 14,
                    className: `text-red-400`
                })
            },
            {
                id: `code-poet`,
                label: `Code Poet`,
                desc: `Focused on highly elegant, expressive, and self-documenting code with beautifully styled comments.`,
                icon: (0, E.jsx)(De, {
                    size: 14,
                    className: `text-purple-400`
                })
            },
            {
                id: `refactor-ninja`,
                label: `Refactor Ninja`,
                desc: `Focused on cleaning up duplicates, simplifying cognitive load, optimizing speed, and refactoring with surgical changes.`,
                icon: (0, E.jsx)(Ae, {
                    size: 14,
                    className: `text-green-400`
                })
            }
        ], g = Math.max(.01, r * 1 + n * .6 + i * .4);
        return (0, E.jsxs)(`div`, {
            className: `flex flex-col gap-6 text-sm h-full overflow-y-auto pr-1`,
            children: [
                (0, E.jsxs)(`div`, {
                    className: `flex flex-col gap-2`,
                    children: [
                        (0, E.jsxs)(`h4`, {
                            className: `text-[10px] uppercase font-bold tracking-wider text-muted-foreground flex items-center gap-1.5`,
                            children: [
                                (0, E.jsx)(ne, {
                                    size: 12
                                }),
                                ` Visual Effects`
                            ]
                        }),
                        (0, E.jsx)(`p`, {
                            className: `text-xs text-muted-foreground mb-1`,
                            children: `Set background Vortex canvas visibility.`
                        }),
                        (0, E.jsx)(`div`, {
                            className: `grid grid-cols-3 gap-2`,
                            children: m.map((n)=>(0, E.jsx)(`button`, {
                                    onClick: ()=>t(n.id),
                                    className: `py-2 px-3 text-xs font-semibold rounded-md border transition-all cursor-pointer ${e === n.id ? `bg-accent border-accent text-background shadow-md font-bold` : `bg-white/5 border-border hover:bg-white/10 text-muted-foreground`}`,
                                    children: n.label
                                }, n.id))
                        })
                    ]
                }),
                (0, E.jsxs)(`div`, {
                    className: `flex flex-col gap-2`,
                    children: [
                        (0, E.jsxs)(`h4`, {
                            className: `text-[10px] uppercase font-bold tracking-wider text-muted-foreground flex items-center gap-1.5`,
                            children: [
                                (0, E.jsx)(pe, {
                                    size: 12
                                }),
                                ` Audio Effects`
                            ]
                        }),
                        (0, E.jsxs)(`div`, {
                            className: `flex items-center justify-between bg-white/5 p-3 rounded-lg border border-white/5`,
                            children: [
                                (0, E.jsxs)(`div`, {
                                    className: `flex flex-col`,
                                    children: [
                                        (0, E.jsx)(`span`, {
                                            className: `text-xs font-bold text-white`,
                                            children: `System Sounds`
                                        }),
                                        (0, E.jsx)(`span`, {
                                            className: `text-[10px] text-muted-foreground`,
                                            children: `Synthesized audio cues for UI interactions`
                                        })
                                    ]
                                }),
                                (0, E.jsxs)(`button`, {
                                    onClick: ()=>p(!s),
                                    className: `px-3 py-1.5 text-xs font-semibold rounded-md border transition-all cursor-pointer flex items-center gap-2 ${s ? `bg-white/5 border-border hover:bg-white/10 text-muted-foreground` : `bg-accent/10 border-accent text-accent shadow-[0_0_10px_rgba(0,242,255,0.15)]`}`,
                                    children: [
                                        s ? (0, E.jsx)(be, {
                                            size: 14
                                        }) : (0, E.jsx)(pe, {
                                            size: 14
                                        }),
                                        s ? `MUTED` : `ENABLED`
                                    ]
                                })
                            ]
                        })
                    ]
                }),
                (0, E.jsxs)(`div`, {
                    className: `flex flex-col gap-2`,
                    children: [
                        (0, E.jsxs)(`h4`, {
                            className: `text-[10px] uppercase font-bold tracking-wider text-muted-foreground flex items-center gap-1.5`,
                            children: [
                                (0, E.jsx)(Ee, {
                                    size: 12
                                }),
                                ` Preset Agent Personas`
                            ]
                        }),
                        (0, E.jsx)(`p`, {
                            className: `text-xs text-muted-foreground mb-1`,
                            children: `Select the operational role guidelines for the agent.`
                        }),
                        (0, E.jsx)(`div`, {
                            className: `flex flex-col gap-2`,
                            children: h.map((e)=>(0, E.jsxs)(`button`, {
                                    onClick: ()=>d(e.id),
                                    className: `p-3 rounded-lg border text-left transition-all duration-300 flex flex-col gap-1 cursor-pointer relative overflow-hidden group ${a === e.id ? `bg-accent/10 border-accent text-white shadow-[0_0_15px_rgba(0,242,255,0.08)]` : `bg-white/5 border-white/5 hover:bg-white/10 hover:border-white/10 text-muted-foreground`}`,
                                    children: [
                                        a === e.id && (0, E.jsx)(`span`, {
                                            className: `absolute top-0 left-0 w-1 h-full bg-accent`
                                        }),
                                        (0, E.jsxs)(`div`, {
                                            className: `flex items-center gap-2 font-bold text-xs text-white`,
                                            children: [
                                                e.icon,
                                                (0, E.jsx)(`span`, {
                                                    className: `group-hover:text-accent transition-colors`,
                                                    children: e.label
                                                })
                                            ]
                                        }),
                                        (0, E.jsx)(`span`, {
                                            className: `text-[11px] opacity-70 leading-normal font-sans`,
                                            children: e.desc
                                        })
                                    ]
                                }, e.id))
                        })
                    ]
                }),
                (0, E.jsxs)(`div`, {
                    className: `flex flex-col gap-2`,
                    children: [
                        (0, E.jsxs)(`h4`, {
                            className: `text-[10px] uppercase font-bold tracking-wider text-muted-foreground flex items-center gap-1.5`,
                            children: [
                                (0, E.jsx)(we, {
                                    size: 12
                                }),
                                ` Inference Settings`
                            ]
                        }),
                        (0, E.jsxs)(`div`, {
                            className: `bg-white/5 p-4 rounded-lg border border-white/5 flex flex-col gap-4`,
                            children: [
                                (0, E.jsxs)(`div`, {
                                    className: `flex justify-between items-center bg-accent/10 border border-accent/20 rounded-md p-2.5 font-mono`,
                                    children: [
                                        (0, E.jsx)(`span`, {
                                            className: `text-xs text-white font-semibold`,
                                            children: `Calculated Temperature:`
                                        }),
                                        (0, E.jsx)(`span`, {
                                            className: `text-accent text-sm font-bold shadow-neon`,
                                            children: g.toFixed(2)
                                        })
                                    ]
                                }),
                                (0, E.jsxs)(`div`, {
                                    children: [
                                        (0, E.jsxs)(`div`, {
                                            className: `flex justify-between items-center text-xs font-mono mb-1 text-muted-foreground`,
                                            children: [
                                                (0, E.jsx)(`span`, {
                                                    children: `Creative vs Precise`
                                                }),
                                                (0, E.jsxs)(`span`, {
                                                    className: `text-white/80 text-[10px]`,
                                                    children: [
                                                        (r * 100).toFixed(0),
                                                        `%`
                                                    ]
                                                })
                                            ]
                                        }),
                                        (0, E.jsxs)(`div`, {
                                            className: `flex items-center gap-2 text-[10px] text-muted-foreground mb-1`,
                                            children: [
                                                (0, E.jsx)(`span`, {
                                                    children: `Precise`
                                                }),
                                                (0, E.jsx)(`input`, {
                                                    type: `range`,
                                                    min: `0.0`,
                                                    max: `1.0`,
                                                    step: `0.05`,
                                                    value: r,
                                                    onChange: (e)=>l(parseFloat(e.target.value)),
                                                    className: `flex-1 accent-accent h-1 bg-white/10 rounded-lg appearance-none cursor-pointer`
                                                }),
                                                (0, E.jsx)(`span`, {
                                                    children: `Creative`
                                                })
                                            ]
                                        })
                                    ]
                                }),
                                (0, E.jsxs)(`div`, {
                                    children: [
                                        (0, E.jsxs)(`div`, {
                                            className: `flex justify-between items-center text-xs font-mono mb-1 text-muted-foreground`,
                                            children: [
                                                (0, E.jsx)(`span`, {
                                                    children: `Aggressive vs Careful`
                                                }),
                                                (0, E.jsxs)(`span`, {
                                                    className: `text-white/80 text-[10px]`,
                                                    children: [
                                                        (n * 100).toFixed(0),
                                                        `%`
                                                    ]
                                                })
                                            ]
                                        }),
                                        (0, E.jsxs)(`div`, {
                                            className: `flex items-center gap-2 text-[10px] text-muted-foreground mb-1`,
                                            children: [
                                                (0, E.jsx)(`span`, {
                                                    children: `Careful`
                                                }),
                                                (0, E.jsx)(`input`, {
                                                    type: `range`,
                                                    min: `0.0`,
                                                    max: `1.0`,
                                                    step: `0.05`,
                                                    value: n,
                                                    onChange: (e)=>c(parseFloat(e.target.value)),
                                                    className: `flex-1 accent-accent h-1 bg-white/10 rounded-lg appearance-none cursor-pointer`
                                                }),
                                                (0, E.jsx)(`span`, {
                                                    children: `Aggressive`
                                                })
                                            ]
                                        })
                                    ]
                                }),
                                (0, E.jsxs)(`div`, {
                                    children: [
                                        (0, E.jsxs)(`div`, {
                                            className: `flex justify-between items-center text-xs font-mono mb-1 text-muted-foreground`,
                                            children: [
                                                (0, E.jsx)(`span`, {
                                                    children: `Fast vs Thorough`
                                                }),
                                                (0, E.jsxs)(`span`, {
                                                    className: `text-white/80 text-[10px]`,
                                                    children: [
                                                        (i * 100).toFixed(0),
                                                        `%`
                                                    ]
                                                })
                                            ]
                                        }),
                                        (0, E.jsxs)(`div`, {
                                            className: `flex items-center gap-2 text-[10px] text-muted-foreground mb-1`,
                                            children: [
                                                (0, E.jsx)(`span`, {
                                                    children: `Thorough`
                                                }),
                                                (0, E.jsx)(`input`, {
                                                    type: `range`,
                                                    min: `0.0`,
                                                    max: `1.0`,
                                                    step: `0.05`,
                                                    value: i,
                                                    onChange: (e)=>u(parseFloat(e.target.value)),
                                                    className: `flex-1 accent-accent h-1 bg-white/10 rounded-lg appearance-none cursor-pointer`
                                                }),
                                                (0, E.jsx)(`span`, {
                                                    children: `Fast`
                                                })
                                            ]
                                        })
                                    ]
                                }),
                                (0, E.jsxs)(`div`, {
                                    className: `border-t border-white/5 pt-3 mt-1`,
                                    children: [
                                        (0, E.jsxs)(`div`, {
                                            className: `flex justify-between items-center text-xs font-mono mb-1 text-muted-foreground`,
                                            children: [
                                                (0, E.jsx)(`span`, {
                                                    children: `Context Limit`
                                                }),
                                                (0, E.jsxs)(`span`, {
                                                    className: `text-accent`,
                                                    children: [
                                                        o,
                                                        ` tokens`
                                                    ]
                                                })
                                            ]
                                        }),
                                        (0, E.jsx)(`input`, {
                                            type: `range`,
                                            min: `2048`,
                                            max: `32768`,
                                            step: `1024`,
                                            value: o,
                                            onChange: (e)=>f(parseInt(e.target.value)),
                                            className: `w-full accent-accent h-1 bg-white/10 rounded-lg appearance-none cursor-pointer`
                                        })
                                    ]
                                })
                            ]
                        })
                    ]
                }),
                (0, E.jsxs)(`div`, {
                    className: `p-3 bg-accent/5 rounded-lg border border-accent/15 flex flex-col gap-2 text-xs text-muted-foreground font-mono mb-4`,
                    children: [
                        (0, E.jsxs)(`div`, {
                            className: `flex items-center gap-2 text-accent font-semibold`,
                            children: [
                                (0, E.jsx)(fe, {
                                    size: 12
                                }),
                                ` System Diagnostics`
                            ]
                        }),
                        (0, E.jsx)(`div`, {
                            children: `Config: local/config.toml`
                        }),
                        (0, E.jsx)(`div`, {
                            children: `UI Frame: React 19.2 + Vite`
                        }),
                        (0, E.jsxs)(`div`, {
                            children: [
                                `Persona: `,
                                a
                            ]
                        })
                    ]
                })
            ]
        });
    }
    function st() {
        let { safeModeRequest: e, setSafeModeRequest: t } = T();
        return e ? (0, E.jsx)(`div`, {
            className: `fixed inset-0 z-[150] flex items-center justify-center p-6 bg-black/60 backdrop-blur-md`,
            children: (0, E.jsxs)(m.div, {
                initial: {
                    opacity: 0,
                    scale: .95
                },
                animate: {
                    opacity: 1,
                    scale: 1
                },
                exit: {
                    opacity: 0,
                    scale: .95
                },
                className: `w-[850px] max-w-full glass-panel border border-border/80 rounded-2xl shadow-[0_0_50px_rgba(0,0,0,0.8)] overflow-hidden flex flex-col max-h-[85vh]`,
                children: [
                    (0, E.jsx)(`div`, {
                        className: `flex items-center justify-between px-6 py-4 border-b border-border/50 bg-black/40`,
                        children: (0, E.jsxs)(`div`, {
                            className: `flex items-center gap-3 text-amber-400`,
                            children: [
                                (0, E.jsx)(ve, {
                                    className: `drop-shadow-[0_0_8px_rgba(251,191,36,0.3)]`
                                }),
                                (0, E.jsxs)(`div`, {
                                    children: [
                                        (0, E.jsx)(`h3`, {
                                            className: `font-semibold text-sm tracking-wider`,
                                            children: `APPROVAL REQUIRED`
                                        }),
                                        (0, E.jsx)(`p`, {
                                            className: `text-[10px] text-muted-foreground uppercase font-bold tracking-widest mt-0.5`,
                                            children: `Safe Mode Sentinel Gate`
                                        })
                                    ]
                                })
                            ]
                        })
                    }),
                    (0, E.jsxs)(`div`, {
                        className: `flex-1 p-6 overflow-y-auto flex flex-col gap-4 min-h-0`,
                        children: [
                            (0, E.jsxs)(`div`, {
                                className: `bg-white/5 border border-white/5 p-4 rounded-xl`,
                                children: [
                                    (0, E.jsx)(`h4`, {
                                        className: `text-[10px] uppercase font-bold tracking-wider text-muted-foreground mb-1`,
                                        children: `Proposed Rationale`
                                    }),
                                    (0, E.jsx)(`p`, {
                                        className: `text-sm leading-relaxed text-white`,
                                        children: e.rationale
                                    })
                                ]
                            }),
                            (0, E.jsxs)(`div`, {
                                className: `flex-1 flex flex-col gap-1 min-h-0`,
                                children: [
                                    (0, E.jsx)(`h4`, {
                                        className: `text-[10px] uppercase font-bold tracking-wider text-muted-foreground mb-1`,
                                        children: `Code Diff`
                                    }),
                                    (0, E.jsx)(`div`, {
                                        className: `flex-1 bg-black/40 border border-border/50 rounded-xl overflow-auto p-4 max-h-[45vh] font-mono text-xs select-text leading-relaxed`,
                                        children: e.diff ? e.diff.split(`
`).map((e, t)=>{
                                            let n = `text-muted-foreground/80`;
                                            return e.startsWith(`+`) ? n = `text-green-400 bg-green-500/10 px-1.5 rounded-sm block w-full py-0.5` : e.startsWith(`-`) ? n = `text-red-400 bg-red-500/10 px-1.5 rounded-sm block w-full py-0.5` : e.startsWith(`@@`) && (n = `text-accent/80 font-bold block py-1 border-t border-white/5 mt-1`), (0, E.jsx)(`span`, {
                                                className: n,
                                                children: e
                                            }, t);
                                        }) : (0, E.jsx)(`span`, {
                                            className: `italic text-muted-foreground`,
                                            children: `No diff payload provided (Permission Escalation)`
                                        })
                                    })
                                ]
                            })
                        ]
                    }),
                    (0, E.jsxs)(`div`, {
                        className: `flex justify-end items-center gap-4 px-6 py-4 border-t border-border/50 bg-black/40 flex-none`,
                        children: [
                            (0, E.jsxs)(`button`, {
                                onClick: ()=>{
                                    window.sendNexus && window.sendNexus(`SafeModeReject`, {}), t(null);
                                },
                                className: `flex items-center gap-2 px-5 py-2.5 rounded-lg border border-red-500/30 bg-red-500/10 text-red-400 text-xs font-bold uppercase tracking-wider hover:bg-red-500/20 active:translate-y-px transition-all cursor-pointer`,
                                children: [
                                    (0, E.jsx)(Te, {
                                        size: 14
                                    }),
                                    ` Reject`
                                ]
                            }),
                            (0, E.jsxs)(`button`, {
                                onClick: ()=>{
                                    Ge(), window.sendNexus && window.sendNexus(`SafeModeApprove`, {}), t(null);
                                },
                                className: `flex items-center gap-2 px-5 py-2.5 rounded-lg bg-accent text-background text-xs font-bold uppercase tracking-wider hover:bg-accent/90 active:translate-y-px transition-all shadow-[0_0_15px_rgba(0,242,255,0.2)] cursor-pointer`,
                                children: [
                                    (0, E.jsx)(de, {
                                        size: 14
                                    }),
                                    ` Approve`
                                ]
                            })
                        ]
                    })
                ]
            })
        }) : null;
    }
    function ct() {
        let { askUserRequest: e, setAskUserRequest: t } = T(), [n, r] = (0, w.useState)(``);
        if (!e) return null;
        let i = (e)=>{
            e && e.preventDefault(), Ge(), window.sendNexus && window.sendNexus(`AskUserResponse`, {
                answer: n
            }), t(null), r(``);
        };
        return (0, E.jsx)(`div`, {
            className: `fixed inset-0 z-[150] flex items-center justify-center p-6 bg-black/60 backdrop-blur-md`,
            children: (0, E.jsxs)(m.div, {
                initial: {
                    opacity: 0,
                    scale: .95
                },
                animate: {
                    opacity: 1,
                    scale: 1
                },
                exit: {
                    opacity: 0,
                    scale: .95
                },
                className: `w-[600px] max-w-full glass-panel border border-border/80 rounded-2xl shadow-[0_0_50px_rgba(0,0,0,0.8)] overflow-hidden flex flex-col max-h-[85vh]`,
                children: [
                    (0, E.jsx)(`div`, {
                        className: `flex items-center justify-between px-6 py-4 border-b border-border/50 bg-black/40`,
                        children: (0, E.jsxs)(`div`, {
                            className: `flex items-center gap-3 text-accent`,
                            children: [
                                (0, E.jsx)(_, {
                                    className: `drop-shadow-[0_0_8px_rgba(0,242,255,0.3)]`
                                }),
                                (0, E.jsxs)(`div`, {
                                    children: [
                                        (0, E.jsx)(`h3`, {
                                            className: `font-semibold text-sm tracking-wider`,
                                            children: `AGENT QUESTION`
                                        }),
                                        (0, E.jsx)(`p`, {
                                            className: `text-[10px] text-muted-foreground uppercase font-bold tracking-widest mt-0.5`,
                                            children: `Clarification Required`
                                        })
                                    ]
                                })
                            ]
                        })
                    }),
                    (0, E.jsxs)(`div`, {
                        className: `flex-1 p-6 flex flex-col gap-4`,
                        children: [
                            (0, E.jsx)(`div`, {
                                className: `bg-white/5 border border-white/5 p-4 rounded-xl`,
                                children: (0, E.jsx)(`p`, {
                                    className: `text-sm leading-relaxed text-white whitespace-pre-wrap`,
                                    children: e.question
                                })
                            }),
                            (0, E.jsxs)(`form`, {
                                onSubmit: i,
                                className: `mt-2 relative`,
                                children: [
                                    (0, E.jsx)(`textarea`, {
                                        autoFocus: !0,
                                        value: n,
                                        onChange: (e)=>r(e.target.value),
                                        onKeyDown: (e)=>{
                                            e.key === `Enter` && !e.shiftKey && (e.preventDefault(), i());
                                        },
                                        placeholder: `Type your response here...`,
                                        className: `w-full h-[120px] bg-black/40 border border-border/50 rounded-xl p-4 text-sm resize-none focus:outline-none focus:border-accent/50 focus:ring-1 focus:ring-accent/50 text-foreground placeholder:text-muted-foreground transition-all`
                                    }),
                                    (0, E.jsx)(`div`, {
                                        className: `absolute bottom-3 right-3 text-[10px] text-muted-foreground/60 select-none pointer-events-none`,
                                        children: `Press Enter to send, Shift+Enter for newline`
                                    })
                                ]
                            })
                        ]
                    }),
                    (0, E.jsx)(`div`, {
                        className: `flex justify-end items-center px-6 py-4 border-t border-border/50 bg-black/40 flex-none`,
                        children: (0, E.jsxs)(`button`, {
                            onClick: ()=>i(),
                            disabled: !n.trim(),
                            className: `flex items-center gap-2 px-5 py-2.5 rounded-lg bg-accent text-background text-xs font-bold uppercase tracking-wider hover:bg-accent/90 active:translate-y-px transition-all shadow-[0_0_15px_rgba(0,242,255,0.2)] disabled:opacity-50 disabled:cursor-not-allowed disabled:shadow-none disabled:active:translate-y-0 cursor-pointer`,
                            children: [
                                (0, E.jsx)(Me, {
                                    size: 14
                                }),
                                ` Send Response`
                            ]
                        })
                    })
                ]
            })
        });
    }
    function lt({ reasoning: e, title: t = `Thought Process`, defaultOpen: n = !1 }) {
        let [r, i] = (0, w.useState)(n);
        return (0, E.jsxs)(`div`, {
            className: `text-xs text-muted-foreground border-l-2 border-accent/50 pl-3 py-1`,
            children: [
                (0, E.jsxs)(`div`, {
                    onClick: ()=>i(!r),
                    className: `cursor-pointer font-semibold select-none hover:text-white transition-colors flex items-center gap-2 w-max`,
                    children: [
                        (0, E.jsx)(ce, {
                            size: 14,
                            className: `text-accent`
                        }),
                        (0, E.jsx)(`span`, {
                            children: t
                        }),
                        (0, E.jsx)(m.span, {
                            animate: {
                                rotate: r ? 90 : 0
                            },
                            transition: {
                                duration: .15
                            },
                            className: `text-[10px] opacity-60 inline-block`,
                            children: `▶`
                        })
                    ]
                }),
                (0, E.jsx)(p, {
                    initial: !1,
                    children: r && (0, E.jsx)(m.div, {
                        initial: {
                            height: 0,
                            opacity: 0,
                            marginTop: 0
                        },
                        animate: {
                            height: `auto`,
                            opacity: 1,
                            marginTop: 8
                        },
                        exit: {
                            height: 0,
                            opacity: 0,
                            marginTop: 0
                        },
                        transition: {
                            duration: .2,
                            ease: `easeInOut`
                        },
                        className: `overflow-hidden`,
                        children: (0, E.jsx)(`div`, {
                            className: `font-mono whitespace-pre-wrap opacity-70 bg-black/20 p-3 rounded`,
                            children: e
                        })
                    })
                })
            ]
        });
    }
    function ut({ tool: e, defaultOpen: t = !1 }) {
        let [n, r] = (0, w.useState)(t);
        return (0, E.jsxs)(`div`, {
            className: `bg-black/20 border border-white/5 rounded block overflow-hidden animate-in fade-in duration-300`,
            children: [
                (0, E.jsxs)(`div`, {
                    onClick: ()=>r(!n),
                    className: `text-xs cursor-pointer select-none py-2 px-3 text-purple-400 font-semibold hover:bg-white/5 transition-colors flex items-center gap-2`,
                    children: [
                        (0, E.jsx)(_e, {
                            size: 14
                        }),
                        (0, E.jsxs)(`span`, {
                            children: [
                                `Executed: `,
                                e.name
                            ]
                        }),
                        (0, E.jsx)(m.span, {
                            animate: {
                                rotate: n ? 90 : 0
                            },
                            transition: {
                                duration: .15
                            },
                            className: `text-[10px] opacity-60 inline-block`,
                            children: `▶`
                        }),
                        e.success ? (0, E.jsx)(g, {
                            size: 12,
                            className: `text-green-500 ml-auto`
                        }) : (0, E.jsx)(ue, {
                            size: 12,
                            className: `text-red-500 ml-auto`
                        })
                    ]
                }),
                (0, E.jsx)(p, {
                    initial: !1,
                    children: n && (0, E.jsx)(m.div, {
                        initial: {
                            height: 0,
                            opacity: 0
                        },
                        animate: {
                            height: `auto`,
                            opacity: 1
                        },
                        exit: {
                            height: 0,
                            opacity: 0
                        },
                        transition: {
                            duration: .2,
                            ease: `easeInOut`
                        },
                        className: `overflow-hidden`,
                        children: (0, E.jsxs)(`div`, {
                            className: `p-3 border-t border-white/5 text-[11px] font-mono`,
                            children: [
                                e.args && (0, E.jsxs)(`div`, {
                                    className: `mb-2`,
                                    children: [
                                        (0, E.jsx)(`strong`, {
                                            className: `text-muted-foreground`,
                                            children: `Input:`
                                        }),
                                        (0, E.jsx)(`pre`, {
                                            className: `whitespace-pre-wrap text-white/70 overflow-x-auto bg-black/40 p-2 mt-1 rounded`,
                                            children: e.args
                                        })
                                    ]
                                }),
                                e.output && (0, E.jsxs)(`div`, {
                                    children: [
                                        (0, E.jsx)(`strong`, {
                                            className: `text-muted-foreground`,
                                            children: `Output:`
                                        }),
                                        (0, E.jsx)(`pre`, {
                                            className: `whitespace-pre-wrap text-white/70 overflow-x-auto max-h-60 bg-black/40 p-2 mt-1 rounded`,
                                            children: e.output
                                        })
                                    ]
                                })
                            ]
                        })
                    })
                })
            ]
        });
    }
    function dt() {
        let { activeToolExecutions: e } = T();
        return !e || e.length === 0 ? null : (0, E.jsxs)(`div`, {
            className: `flex flex-col gap-3 my-4 p-4 rounded-xl border border-white/5 bg-white/[0.02] backdrop-blur-sm animate-in fade-in slide-in-from-bottom-2 duration-300`,
            children: [
                (0, E.jsxs)(`div`, {
                    className: `flex items-center justify-between`,
                    children: [
                        (0, E.jsxs)(`h4`, {
                            className: `text-xs font-bold text-accent uppercase tracking-wider flex items-center gap-2`,
                            children: [
                                (0, E.jsx)(`span`, {
                                    className: `w-1.5 h-1.5 rounded-full bg-accent animate-ping`
                                }),
                                `Parallel Execution Streams (`,
                                e.filter((e)=>e.status === `running`).length,
                                ` Active)`
                            ]
                        }),
                        (0, E.jsx)(`div`, {
                            className: `text-[10px] text-muted-foreground font-mono`,
                            children: `Sync: Concurrent Threading`
                        })
                    ]
                }),
                (0, E.jsx)(`div`, {
                    className: `grid grid-cols-1 md:grid-cols-2 gap-4`,
                    children: e.map((e)=>(0, E.jsxs)(m.div, {
                            layout: !0,
                            initial: {
                                opacity: 0,
                                scale: .95
                            },
                            animate: {
                                opacity: 1,
                                scale: 1
                            },
                            exit: {
                                opacity: 0,
                                scale: .95
                            },
                            transition: {
                                duration: .3
                            },
                            className: `relative flex flex-col gap-2 p-4 rounded-xl border backdrop-blur-md transition-all duration-300 shadow-xl ${e.status === `running` ? `bg-blue-500/5 border-blue-500/20 shadow-blue-500/5` : e.status === `success` ? `bg-green-500/5 border-green-500/20 shadow-green-500/5` : `bg-red-500/5 border-red-500/20 shadow-red-500/5`}`,
                            children: [
                                (0, E.jsx)(`div`, {
                                    className: `absolute top-0 right-0 w-24 h-24 rounded-full filter blur-[40px] opacity-10 pointer-events-none -mr-8 -mt-8 ${e.status === `running` ? `bg-blue-500` : e.status === `success` ? `bg-green-500` : `bg-red-500`}`
                                }),
                                (0, E.jsxs)(`div`, {
                                    className: `flex items-start justify-between`,
                                    children: [
                                        (0, E.jsxs)(`div`, {
                                            className: `flex flex-col min-w-0`,
                                            children: [
                                                (0, E.jsxs)(`span`, {
                                                    className: `text-xs font-bold text-white font-mono flex items-center gap-1.5 truncate`,
                                                    children: [
                                                        `⚙️ `,
                                                        e.name
                                                    ]
                                                }),
                                                e.args && (0, E.jsx)(`span`, {
                                                    className: `text-[9px] text-muted-foreground font-mono truncate max-w-[180px] mt-0.5`,
                                                    title: e.args,
                                                    children: e.args
                                                })
                                            ]
                                        }),
                                        (0, E.jsx)(`span`, {
                                            className: `text-[9px] px-2 py-0.5 rounded-full font-bold uppercase shrink-0 ${e.status === `running` ? `bg-blue-500/25 text-blue-400 animate-pulse` : e.status === `success` ? `bg-green-500/25 text-green-400` : `bg-red-500/25 text-red-400`}`,
                                            children: e.status
                                        })
                                    ]
                                }),
                                e.status === `running` ? (0, E.jsxs)(`div`, {
                                    className: `mt-2 flex flex-col gap-1`,
                                    children: [
                                        (0, E.jsx)(`div`, {
                                            className: `h-1 w-full bg-white/5 rounded-full overflow-hidden`,
                                            children: (0, E.jsx)(m.div, {
                                                className: `h-full bg-gradient-to-r from-blue-500 to-indigo-500 rounded-full`,
                                                initial: {
                                                    width: `15%`
                                                },
                                                animate: {
                                                    width: `90%`
                                                },
                                                transition: {
                                                    duration: 12,
                                                    ease: `easeOut`
                                                }
                                            })
                                        }),
                                        (0, E.jsx)(`span`, {
                                            className: `text-[8px] text-blue-400/80 font-mono animate-pulse`,
                                            children: `Streaming execution thread...`
                                        })
                                    ]
                                }) : (0, E.jsx)(`div`, {
                                    className: `mt-2 flex flex-col gap-1.5`,
                                    children: (0, E.jsxs)(`details`, {
                                        className: `text-[9px] text-muted-foreground font-mono bg-black/40 rounded border border-white/5 overflow-hidden`,
                                        children: [
                                            (0, E.jsx)(`summary`, {
                                                className: `cursor-pointer py-1 px-2 select-none hover:bg-white/5 font-semibold text-[8px] text-white/60`,
                                                children: `Show Log Output`
                                            }),
                                            (0, E.jsx)(`div`, {
                                                className: `p-2 border-t border-white/5 max-h-32 overflow-y-auto whitespace-pre-wrap text-[9px] text-white/80 leading-normal select-text`,
                                                children: e.output || `No output log.`
                                            })
                                        ]
                                    })
                                })
                            ]
                        }, e.id))
                })
            ]
        });
    }
    function ft() {
        let { messages: e, isStreaming: t, addMessage: n, streamAccumulator: r, reasoningAccumulator: i, currentToolResults: a } = T(), [o, s] = (0, w.useState)(``), [c, l] = (0, w.useState)(null), u = (0, w.useRef)(null);
        (0, w.useEffect)(()=>{
            u.current && (u.current.scrollTop = u.current.scrollHeight);
        }, [
            e,
            t,
            r,
            i,
            a
        ]);
        let d = ()=>{
            if (!(!o.trim() || t)) {
                if (n({
                    id: Date.now().toString(),
                    role: `user`,
                    content: o
                }), window.sendNexus) {
                    let e, t = T.getState(), { activeFile: n } = t;
                    n && (e = `${n.name}\n\nFile Contents:\n\`\`\`${n.ext}\n${n.content}\n\`\`\``);
                    let r = Math.max(.01, t.sliderCreativePrecise * 1 + t.sliderAggressiveCareful * .6 + t.sliderFastThorough * .4);
                    window.sendNexus(`Chat`, {
                        message: o,
                        editor_context: e,
                        temperature: r,
                        context_limit: t.contextLimit,
                        role: t.activeRole
                    }), T.getState().clearActiveToolExecutions(), T.getState().setStreaming(!0);
                }
                s(``);
            }
        }, f = ()=>{
            window.sendNexus && window.sendNexus(`StopStream`, {}), T.getState().commitStream();
        }, h = (t)=>{
            let n = e.filter((e)=>e.role === `user`).findIndex((e)=>e.id === t.id);
            if (n !== -1) {
                let r = e.findIndex((e)=>e.id === t.id);
                if (r !== -1) {
                    let t = e.slice(0, r + 1);
                    T.getState().setMessages(t);
                }
                window.sendNexus && window.sendNexus(`RollbackHistory`, {
                    user_message_index: n
                });
            }
            l(null);
        };
        return (0, E.jsxs)(`div`, {
            className: `flex flex-col h-full w-full bg-background relative`,
            children: [
                (0, E.jsxs)(`div`, {
                    ref: u,
                    className: `flex-1 overflow-y-auto p-6 flex flex-col relative`,
                    children: [
                        (0, E.jsx)(`div`, {
                            className: `absolute left-10 top-6 bottom-6 w-0.5 bg-white/10 z-0`
                        }),
                        (0, E.jsxs)(`div`, {
                            className: `flex flex-col gap-6 z-10 relative`,
                            children: [
                                (0, E.jsx)(p, {
                                    children: e.map((e)=>(0, E.jsxs)(m.div, {
                                            initial: {
                                                opacity: 0,
                                                x: -20
                                            },
                                            animate: {
                                                opacity: 1,
                                                x: 0
                                            },
                                            className: `flex gap-4 relative group`,
                                            children: [
                                                (0, E.jsx)(`div`, {
                                                    className: `flex-shrink-0 w-8 h-8 rounded-full flex items-center justify-center bg-background border-2 shadow-sm z-10 
                  ${e.role === `user` ? `border-blue-500 text-blue-500` : e.role === `system` ? `border-accent text-accent` : `border-green-500 text-green-500`}
                `,
                                                    children: e.role === `user` ? (0, E.jsx)(Ie, {
                                                        size: 16
                                                    }) : e.role === `system` ? (0, E.jsx)(ae, {
                                                        size: 16
                                                    }) : (0, E.jsx)(g, {
                                                        size: 16
                                                    })
                                                }),
                                                (0, E.jsxs)(`div`, {
                                                    className: `flex-1 flex flex-col gap-2 relative ${e.role === `user` ? `pt-1` : ``}`,
                                                    children: [
                                                        e.role === `user` && e.id !== `init` && (0, E.jsx)(`div`, {
                                                            className: `absolute top-1 right-2 flex items-center gap-2 opacity-0 group-hover:opacity-100 transition-opacity z-20`,
                                                            children: (0, E.jsxs)(`button`, {
                                                                onClick: ()=>l(e.id),
                                                                className: `flex items-center gap-1.5 px-2 py-1 bg-accent/20 hover:bg-accent/40 border border-accent/30 rounded text-[10px] text-accent font-bold cursor-pointer transition-colors shadow-sm`,
                                                                title: `Rewind conversation to here`,
                                                                children: [
                                                                    (0, E.jsx)(me, {
                                                                        size: 10
                                                                    }),
                                                                    ` REWIND`
                                                                ]
                                                            })
                                                        }),
                                                        c === e.id && (0, E.jsxs)(`div`, {
                                                            className: `absolute inset-0 bg-background/90 backdrop-blur-md rounded-xl flex items-center justify-between px-6 z-30 border border-accent/30 animate-in fade-in duration-200`,
                                                            children: [
                                                                (0, E.jsxs)(`div`, {
                                                                    className: `flex items-center gap-2 text-xs font-mono text-white/90`,
                                                                    children: [
                                                                        (0, E.jsx)(me, {
                                                                            size: 14,
                                                                            className: `text-accent animate-pulse`
                                                                        }),
                                                                        (0, E.jsx)(`span`, {
                                                                            children: `Rewind session to this prompt? (LLM context will be reset).`
                                                                        })
                                                                    ]
                                                                }),
                                                                (0, E.jsxs)(`div`, {
                                                                    className: `flex gap-2`,
                                                                    children: [
                                                                        (0, E.jsx)(`button`, {
                                                                            onClick: ()=>h(e),
                                                                            className: `bg-accent hover:bg-accent/90 text-background px-3 py-1 rounded text-xs font-bold transition-all cursor-pointer shadow-md`,
                                                                            children: `CONFIRM`
                                                                        }),
                                                                        (0, E.jsx)(`button`, {
                                                                            onClick: ()=>l(null),
                                                                            className: `bg-white/10 hover:bg-white/20 text-white px-3 py-1 rounded text-xs font-bold transition-all cursor-pointer border border-white/10`,
                                                                            children: `CANCEL`
                                                                        })
                                                                    ]
                                                                })
                                                            ]
                                                        }),
                                                        e.role === `user` && (0, E.jsx)(`div`, {
                                                            className: `text-sm font-semibold text-white/90`,
                                                            children: `User`
                                                        }),
                                                        e.role === `ai` && (0, E.jsx)(`div`, {
                                                            className: `text-sm font-semibold text-green-400`,
                                                            children: `Tempest Agent`
                                                        }),
                                                        e.role === `system` && (0, E.jsx)(`div`, {
                                                            className: `text-sm font-semibold text-accent`,
                                                            children: `System`
                                                        }),
                                                        e.reasoning && (0, E.jsx)(lt, {
                                                            reasoning: e.reasoning
                                                        }),
                                                        e.tools && e.tools.length > 0 && (0, E.jsx)(`div`, {
                                                            className: `flex flex-col gap-2 border-l-2 border-purple-500/50 pl-3 py-1`,
                                                            children: e.tools.map((e, t)=>(0, E.jsx)(ut, {
                                                                    tool: e
                                                                }, t))
                                                        }),
                                                        e.content && (0, E.jsx)(`div`, {
                                                            className: `text-sm leading-relaxed whitespace-pre-wrap p-4 rounded-xl border ${e.role === `user` ? `bg-blue-500/10 border-blue-500/20 text-white/90` : e.role === `system` ? `bg-accent/10 border-accent/20 font-mono text-accent` : `bg-green-500/10 border-green-500/20 text-white/90`}`,
                                                            children: e.content
                                                        })
                                                    ]
                                                })
                                            ]
                                        }, e.id))
                                }),
                                t && (0, E.jsxs)(m.div, {
                                    initial: {
                                        opacity: 0,
                                        x: -20
                                    },
                                    animate: {
                                        opacity: 1,
                                        x: 0
                                    },
                                    className: `flex gap-4 relative`,
                                    children: [
                                        (0, E.jsx)(`div`, {
                                            className: `flex-shrink-0 w-8 h-8 rounded-full flex items-center justify-center bg-background border-2 shadow-sm z-10 border-accent text-accent animate-pulse`,
                                            children: (0, E.jsx)(ae, {
                                                size: 16
                                            })
                                        }),
                                        (0, E.jsxs)(`div`, {
                                            className: `flex-1 flex flex-col gap-2`,
                                            children: [
                                                (0, E.jsx)(`div`, {
                                                    className: `text-sm font-semibold text-accent animate-pulse`,
                                                    children: `Tempest Agent (Active)`
                                                }),
                                                i && (0, E.jsx)(lt, {
                                                    reasoning: i,
                                                    title: `Thinking...`,
                                                    defaultOpen: !0
                                                }),
                                                (0, E.jsx)(dt, {}),
                                                a.length > 0 && (0, E.jsx)(`div`, {
                                                    className: `flex flex-col gap-2 border-l-2 border-purple-500/50 pl-3 py-1`,
                                                    children: a.map((e, t)=>(0, E.jsx)(ut, {
                                                            tool: e,
                                                            defaultOpen: !0
                                                        }, t))
                                                }),
                                                r && (0, E.jsx)(`div`, {
                                                    className: `text-sm leading-relaxed whitespace-pre-wrap p-4 rounded-xl border bg-accent/10 border-accent/20 text-white/90 animate-pulse`,
                                                    children: r
                                                })
                                            ]
                                        })
                                    ]
                                })
                            ]
                        })
                    ]
                }),
                (0, E.jsx)(`div`, {
                    className: `p-4 bg-black/20 border-t border-border/50 relative z-20`,
                    children: (0, E.jsxs)(`div`, {
                        className: `flex gap-3`,
                        children: [
                            (0, E.jsx)(`input`, {
                                value: o,
                                onChange: (e)=>s(e.target.value),
                                onKeyDown: (e)=>e.key === `Enter` && d(),
                                className: `flex-1 bg-white/5 border border-border/50 rounded-lg px-4 py-3 text-sm focus:outline-none focus:border-accent transition-colors shadow-inner text-white placeholder-muted-foreground`,
                                placeholder: `Enter objective...`,
                                disabled: t
                            }),
                            (0, E.jsx)(`button`, {
                                onClick: t ? f : d,
                                disabled: !t && !o,
                                className: `font-bold px-6 py-3 rounded-lg flex items-center justify-center transition-all shadow-lg hover:-translate-y-0.5 active:translate-y-0 ${t ? `bg-destructive hover:bg-destructive/90 text-white` : `bg-accent hover:bg-accent/90 text-background`}`,
                                children: t ? (0, E.jsx)(ge, {
                                    size: 18,
                                    className: `fill-current`
                                }) : (0, E.jsx)(Me, {
                                    size: 18
                                })
                            })
                        ]
                    })
                })
            ]
        });
    }
    var pt = (e)=>({
            rs: `rust`,
            zig: `zig`,
            ts: `typescript`,
            tsx: `typescript`,
            js: `javascript`,
            jsx: `javascript`,
            sh: `shell`,
            bash: `shell`,
            nix: `nix`,
            toml: `toml`,
            lock: `toml`,
            md: `markdown`,
            json: `json`,
            html: `html`,
            css: `css`,
            py: `python`,
            yml: `yaml`,
            yaml: `yaml`,
            c: `c`,
            cpp: `cpp`,
            h: `cpp`,
            txt: `plaintext`
        })[e.split(`.`).pop()?.toLowerCase() || ``] || `plaintext`;
    function mt() {
        let { turnReviewRequest: e, setTurnReviewRequest: t } = T(), [n, r] = (0, w.useState)(0);
        if (!e || e.files.length === 0) return null;
        let i = e.files[n] || e.files[0];
        return (0, E.jsx)(`div`, {
            className: `fixed inset-0 z-[150] flex items-center justify-center p-6 bg-black/70 backdrop-blur-md`,
            children: (0, E.jsxs)(m.div, {
                initial: {
                    opacity: 0,
                    scale: .97
                },
                animate: {
                    opacity: 1,
                    scale: 1
                },
                exit: {
                    opacity: 0,
                    scale: .97
                },
                className: `w-[1250px] max-w-full h-[85vh] glass-panel border border-border/80 rounded-2xl shadow-[0_0_60px_rgba(0,0,0,0.8)] overflow-hidden flex flex-col`,
                children: [
                    (0, E.jsxs)(`div`, {
                        className: `flex items-center justify-between px-6 py-4 border-b border-border/50 bg-black/40 flex-none`,
                        children: [
                            (0, E.jsxs)(`div`, {
                                className: `flex items-center gap-3`,
                                children: [
                                    (0, E.jsx)(`div`, {
                                        className: `w-10 h-10 rounded-lg bg-accent/10 border border-accent/30 flex items-center justify-center text-accent`,
                                        children: (0, E.jsx)(x, {
                                            size: 20,
                                            className: `drop-shadow-[0_0_6px_rgba(0,242,255,0.4)]`
                                        })
                                    }),
                                    (0, E.jsxs)(`div`, {
                                        children: [
                                            (0, E.jsx)(`h3`, {
                                                className: `font-semibold text-sm tracking-wider text-white`,
                                                children: `TURN COMPLETION REVIEW`
                                            }),
                                            (0, E.jsx)(`p`, {
                                                className: `text-[10px] text-muted-foreground uppercase font-bold tracking-widest mt-0.5`,
                                                children: `Verify and commit code changes from the agent's turn`
                                            })
                                        ]
                                    })
                                ]
                            }),
                            (0, E.jsxs)(`div`, {
                                className: `flex items-center gap-2 bg-amber-500/10 border border-amber-500/20 px-3 py-1.5 rounded-lg text-amber-400 font-mono text-[10px] uppercase`,
                                children: [
                                    (0, E.jsx)(ke, {
                                        size: 12
                                    }),
                                    (0, E.jsx)(`span`, {
                                        children: `Workspace Changes Pending Approval`
                                    })
                                ]
                            })
                        ]
                    }),
                    (0, E.jsxs)(`div`, {
                        className: `flex-1 min-h-0 flex flex-row`,
                        children: [
                            (0, E.jsxs)(`div`, {
                                className: `w-[320px] flex-shrink-0 border-r border-border/50 bg-black/25 flex flex-col`,
                                children: [
                                    (0, E.jsx)(`div`, {
                                        className: `p-4 border-b border-border/30 bg-white/[0.01]`,
                                        children: (0, E.jsxs)(`span`, {
                                            className: `text-[10px] font-bold text-muted-foreground uppercase tracking-widest`,
                                            children: [
                                                `Modified Files (`,
                                                e.files.length,
                                                `)`
                                            ]
                                        })
                                    }),
                                    (0, E.jsx)(`div`, {
                                        className: `flex-1 overflow-y-auto p-2 flex flex-col gap-1`,
                                        children: e.files.map((e, t)=>{
                                            let i = e.path.split(`/`).pop() || e.path, a = e.path.split(`/`).slice(0, -1).join(`/`), o = n === t;
                                            return (0, E.jsxs)(`button`, {
                                                onClick: ()=>r(t),
                                                className: `w-full text-left p-3 rounded-xl border transition-all duration-200 flex items-start gap-3 cursor-pointer ${o ? `bg-accent/10 border-accent/40 text-white` : `bg-transparent border-transparent hover:bg-white/5 text-muted-foreground hover:text-white`}`,
                                                children: [
                                                    (0, E.jsx)(Se, {
                                                        size: 16,
                                                        className: `mt-0.5 ${o ? `text-accent` : `text-muted-foreground/60`}`
                                                    }),
                                                    (0, E.jsxs)(`div`, {
                                                        className: `flex-1 min-w-0 flex flex-col gap-0.5`,
                                                        children: [
                                                            (0, E.jsx)(`span`, {
                                                                className: `text-xs font-bold font-mono truncate`,
                                                                children: i
                                                            }),
                                                            a && (0, E.jsx)(`span`, {
                                                                className: `text-[9px] font-mono opacity-50 truncate`,
                                                                children: a
                                                            })
                                                        ]
                                                    }),
                                                    (0, E.jsx)(te, {
                                                        size: 14,
                                                        className: `mt-1 transition-transform ${o ? `text-accent translate-x-0.5` : `opacity-0`}`
                                                    })
                                                ]
                                            }, t);
                                        })
                                    })
                                ]
                            }),
                            (0, E.jsxs)(`div`, {
                                className: `flex-1 min-w-0 flex flex-col bg-black/10`,
                                children: [
                                    (0, E.jsxs)(`div`, {
                                        className: `px-6 py-3 border-b border-border/30 bg-white/[0.01] flex items-center justify-between`,
                                        children: [
                                            (0, E.jsxs)(`span`, {
                                                className: `text-[10px] font-bold font-mono text-muted-foreground`,
                                                children: [
                                                    `PATH: `,
                                                    (0, E.jsx)(`span`, {
                                                        className: `text-white`,
                                                        children: i?.path
                                                    })
                                                ]
                                            }),
                                            (0, E.jsx)(`span`, {
                                                className: `text-[9px] font-mono bg-purple-500/10 text-purple-400 border border-purple-500/20 px-2 py-0.5 rounded uppercase`,
                                                children: pt(i?.path || ``)
                                            })
                                        ]
                                    }),
                                    (0, E.jsx)(`div`, {
                                        className: `flex-1 min-h-0 select-text`,
                                        children: i ? (0, E.jsx)(Le, {
                                            height: `100%`,
                                            language: pt(i.path),
                                            theme: `vs-dark`,
                                            original: i.original,
                                            modified: i.modified,
                                            options: {
                                                readOnly: !0,
                                                minimap: {
                                                    enabled: !1
                                                },
                                                fontSize: 12,
                                                fontFamily: `"JetBrains Mono", monospace`,
                                                scrollBeyondLastLine: !1,
                                                renderSideBySide: !0,
                                                smoothScrolling: !0
                                            },
                                            loading: (0, E.jsx)(`div`, {
                                                className: `flex items-center justify-center h-full text-accent font-mono text-xs animate-pulse`,
                                                children: `Generating side-by-side comparison...`
                                            })
                                        }) : (0, E.jsx)(`div`, {
                                            className: `flex items-center justify-center h-full text-muted-foreground text-xs font-mono`,
                                            children: `Select a file from the sidebar to inspect diff details.`
                                        })
                                    })
                                ]
                            })
                        ]
                    }),
                    (0, E.jsxs)(`div`, {
                        className: `px-6 py-4 border-t border-border/50 bg-black/40 flex items-center justify-between flex-none`,
                        children: [
                            (0, E.jsx)(`span`, {
                                className: `text-[10px] text-muted-foreground/60 font-mono`,
                                children: `Pro Tip: Click 'Tweak' to close review and make manual edits in the main panel.`
                            }),
                            (0, E.jsxs)(`div`, {
                                className: `flex items-center gap-3`,
                                children: [
                                    (0, E.jsxs)(`button`, {
                                        onClick: ()=>{
                                            i && (window.sendNexus && (window.sendNexus(`ReadFile`, {
                                                path: i.path
                                            }), T.getState().setFileEditable(!0), T.getState().setEditorFocused(!0)), t(null));
                                        },
                                        className: `flex items-center gap-2 px-4 py-2.5 rounded-lg border border-white/10 hover:border-white/20 bg-white/5 text-white text-xs font-bold uppercase tracking-wider hover:bg-white/10 active:translate-y-px transition-all cursor-pointer`,
                                        children: [
                                            (0, E.jsx)(x, {
                                                size: 14
                                            }),
                                            ` Tweak Code`
                                        ]
                                    }),
                                    (0, E.jsxs)(`button`, {
                                        onClick: ()=>{
                                            window.sendNexus && window.sendNexus(`ReviewReject`, {}), t(null);
                                        },
                                        className: `flex items-center gap-2 px-4 py-2.5 rounded-lg border border-red-500/30 bg-red-500/10 text-red-400 text-xs font-bold uppercase tracking-wider hover:bg-red-500/20 active:translate-y-px transition-all cursor-pointer`,
                                        children: [
                                            (0, E.jsx)(Te, {
                                                size: 14
                                            }),
                                            ` Reject All`
                                        ]
                                    }),
                                    (0, E.jsxs)(`button`, {
                                        onClick: ()=>{
                                            window.sendNexus && window.sendNexus(`ReviewApprove`, {}), t(null);
                                        },
                                        className: `flex items-center gap-2 px-5 py-2.5 rounded-lg bg-accent text-background text-xs font-bold uppercase tracking-wider hover:bg-accent/90 active:translate-y-px transition-all shadow-[0_0_15px_rgba(0,242,255,0.2)] cursor-pointer`,
                                        children: [
                                            (0, E.jsx)(de, {
                                                size: 14
                                            }),
                                            ` Accept & Commit`
                                        ]
                                    })
                                ]
                            })
                        ]
                    })
                ]
            })
        });
    }
    function ht() {
        let e = (0, w.useRef)(null), t = (t, n)=>{
            e.current && e.current.readyState === WebSocket.OPEN && e.current.send(JSON.stringify({
                type: t,
                payload: n
            }));
        };
        return (0, w.useEffect)(()=>{
            window.sendNexus = t;
            let n, r = ()=>{
                let a = new WebSocket(`ws://localhost:8080/ws`);
                e.current = a, a.onopen = ()=>{
                    console.log(`📡 [NEXUS]: Connection established.`), T.getState().setConnected(!0), t(`ListFiles`, {
                        path: `.`
                    }), t(`GetHistory`, {}), t(`GetMemories`, {});
                }, a.onclose = ()=>{
                    console.log(`❌ [NEXUS]: Connection lost. Retrying...`);
                    let e = T.getState();
                    e.setConnected(!1), e.setStreaming(!1), n = setTimeout(r, 2e3);
                }, a.onmessage = (e)=>{
                    i(JSON.parse(e.data));
                };
            }, i = (e)=>{
                let n = T.getState();
                switch(e.type){
                    case `History`:
                        n.setMessages(e.payload.messages || []);
                        break;
                    case `StreamToken`:
                        n.appendStreamContent(e.payload.token);
                        break;
                    case `ReasoningToken`:
                        n.appendReasoningContent(e.payload.token);
                        break;
                    case `Done`:
                        n.commitStream();
                        break;
                    case `Telemetry`:
                        n.setMetrics(e.payload.cpu, e.payload.gpu, `${e.payload.ram}`);
                        break;
                    case `InferenceMetrics`:
                        e.payload.tps != null && n.setTps(`${e.payload.tps} t/s`), e.payload.ctx_used != null && n.setCtxUsed(e.payload.ctx_used), e.payload.ctx_total != null && n.setCtxTotal(e.payload.ctx_total), e.payload.kv_cache_hit_pct != null && n.setKvCacheHitPct(e.payload.kv_cache_hit_pct), (e.payload.planning_duration_ms != null || e.payload.executing_duration_ms != null || e.payload.verifying_duration_ms != null) && n.setPhaseDurations(e.payload.planning_duration_ms ?? null, e.payload.executing_duration_ms ?? null, e.payload.verifying_duration_ms ?? null);
                        break;
                    case `FileTree`:
                        n.setExplorer(e.payload.current_path, e.payload.items);
                        break;
                    case `FileContent`:
                        {
                            let t = e.payload.path || `unknown`, r = t.split(`.`).pop() || ``;
                            n.setActiveFile({
                                name: t,
                                content: e.payload.content,
                                ext: r
                            });
                            break;
                        }
                    case `TerminalOutput`:
                        Je(), window.dispatchEvent(new CustomEvent(`terminal-output`, {
                            detail: e.payload.data
                        }));
                        break;
                    case `BackendInfo`:
                        n.setBackendInfo(e.payload.backend, e.payload.planner, e.payload.executor, e.payload.verifier);
                        break;
                    case `AgentStateChange`:
                        n.setAgentPhase(e.payload.state), e.payload.state === `Done` && (n.setActiveTools([]), n.setStreaming(!1));
                        break;
                    case `ActiveTools`:
                        n.setActiveTools(e.payload.tools);
                        break;
                    case `ToolStart`:
                        e.payload.name === `run_command` ? Ke() : He(), n.addActiveToolExecution(e.payload.name, e.payload.args);
                        break;
                    case `ToolResult`:
                        e.payload.success ? Ue() : We(), n.addToolResult({
                            name: e.payload.name,
                            args: e.payload.args,
                            output: e.payload.output,
                            success: e.payload.success
                        }), n.updateActiveToolExecution(e.payload.name, e.payload.args, e.payload.success ? `success` : `error`, e.payload.output), e.payload.name === `store_memory` && e.payload.success && t(`GetMemories`, {});
                        break;
                    case `SafeModeRequest`:
                        console.log(`🔒 [NEXUS]: SafeModeRequest received`, e.payload), n.setSafeModeRequest({
                            rationale: e.payload.rationale,
                            diff: e.payload.diff
                        });
                        break;
                    case `AskUserRequest`:
                        console.log(`❓ [NEXUS]: AskUserRequest received`, e.payload), n.setAskUserRequest({
                            question: e.payload.question
                        });
                        break;
                    case `Memories`:
                        n.setMemories(e.payload.memories || []);
                        break;
                    case `TurnReviewRequest`:
                        console.log(`🔍 [NEXUS]: TurnReviewRequest received`, e.payload), n.setTurnReviewRequest({
                            diff: e.payload.diff,
                            files: e.payload.files || []
                        });
                        break;
                    case `SearchResults`:
                        n.setSearchResults(e.payload.matches || []), n.setSearching(!1);
                        break;
                    case `Error`:
                        n.appendStreamContent(`\n\n**System Error:** ${e.payload.message}\n`), n.commitStream(), n.clearReasoning(), n.setStreaming(!1);
                        break;
                }
            };
            return r(), ()=>{
                clearTimeout(n), e.current && e.current.close();
            };
        }, []), {
            sendNexus: t
        };
    }
    var gt = `/assets/tempest_wasm_bg-B8Kuvi3L.wasm`, _t = async (e = {}, t)=>{
        let n;
        if (t.startsWith(`data:`)) {
            let r = t.replace(/^data:.*?base64,/, ``), i;
            if (typeof Buffer == `function` && typeof Buffer.from == `function`) i = Buffer.from(r, `base64`);
            else if (typeof atob == `function`) {
                let e = atob(r);
                i = new Uint8Array(e.length);
                for(let t = 0; t < e.length; t++)i[t] = e.charCodeAt(t);
            } else throw Error(`Cannot decode base64-encoded data URL`);
            n = await WebAssembly.instantiate(i, e);
        } else {
            let r = await fetch(t), i = r.headers.get(`Content-Type`) || ``;
            if (`instantiateStreaming` in WebAssembly && i.startsWith(`application/wasm`)) n = await WebAssembly.instantiateStreaming(r, e);
            else {
                let t = await r.arrayBuffer();
                n = await WebAssembly.instantiate(t, e);
            }
        }
        return n.instance.exports;
    }, vt = class e {
        static __wrap(t) {
            let n = Object.create(e.prototype);
            return n.__wbg_ptr = t, Uu.register(n, n.__wbg_ptr, n), n;
        }
        __destroy_into_raw() {
            let e = this.__wbg_ptr;
            return this.__wbg_ptr = 0, Uu.unregister(this), e;
        }
        free() {
            let e = this.__destroy_into_raw();
            $.__wbg_dashboard_free(e, 0);
        }
        render() {
            $.dashboard_render(this.__wbg_ptr);
        }
        resize(e, t) {
            $.dashboard_resize(this.__wbg_ptr, e, t);
        }
    };
    Symbol.dispose && (vt.prototype[Symbol.dispose] = vt.prototype.free);
    function yt(e) {
        let t = Y(e, $.__wbindgen_malloc, $.__wbindgen_realloc), n = Q;
        return $.initialize_dashboard(t, n);
    }
    function bt(e) {
        return e.Window;
    }
    function xt(e) {
        return e.WorkerGlobalScope;
    }
    function St(e) {
        let t = e, n = typeof t == `boolean` ? t : void 0;
        return J(n) ? 16777215 : +!!n;
    }
    function Ct(e, t) {
        let n = Y(Gu(t), $.__wbindgen_malloc, $.__wbindgen_realloc), r = Q;
        R().setInt32(e + 4, r, !0), R().setInt32(e + 0, n, !0);
    }
    function wt(e) {
        return typeof e == `function`;
    }
    function Tt(e) {
        return e === null;
    }
    function Et(e) {
        return e === void 0;
    }
    function Dt(e, t) {
        let n = t, r = typeof n == `number` ? n : void 0;
        R().setFloat64(e + 8, J(r) ? 0 : r, !0), R().setInt32(e + 0, !J(r), !0);
    }
    function Ot(e, t) {
        let n = t, r = typeof n == `string` ? n : void 0;
        var i = J(r) ? 0 : Y(r, $.__wbindgen_malloc, $.__wbindgen_realloc), a = Q;
        R().setInt32(e + 4, a, !0), R().setInt32(e + 0, i, !0);
    }
    function kt(e, t) {
        throw Error(U(e, t));
    }
    function At(e) {
        e._wbg_cb_unref();
    }
    function jt(e, t) {
        e.activeTexture(t >>> 0);
    }
    function Mt(e, t) {
        e.activeTexture(t >>> 0);
    }
    function Nt(e, t, n) {
        e.attachShader(t, n);
    }
    function Pt(e, t, n) {
        e.attachShader(t, n);
    }
    function Ft(e, t, n) {
        e.beginQuery(t >>> 0, n);
    }
    function It() {
        return q(function(e, t) {
            return e.beginRenderPass(t);
        }, arguments);
    }
    function Lt(e, t, n, r, i) {
        e.bindAttribLocation(t, n >>> 0, U(r, i));
    }
    function Rt(e, t, n, r, i) {
        e.bindAttribLocation(t, n >>> 0, U(r, i));
    }
    function zt(e, t, n, r, i, a) {
        e.bindBufferRange(t >>> 0, n >>> 0, r, i, a);
    }
    function Bt(e, t, n) {
        e.bindBuffer(t >>> 0, n);
    }
    function Vt(e, t, n) {
        e.bindBuffer(t >>> 0, n);
    }
    function Ht(e, t, n) {
        e.bindFramebuffer(t >>> 0, n);
    }
    function Ut(e, t, n) {
        e.bindFramebuffer(t >>> 0, n);
    }
    function Wt(e, t, n) {
        e.bindRenderbuffer(t >>> 0, n);
    }
    function Gt(e, t, n) {
        e.bindRenderbuffer(t >>> 0, n);
    }
    function Kt(e, t, n) {
        e.bindSampler(t >>> 0, n);
    }
    function qt(e, t, n) {
        e.bindTexture(t >>> 0, n);
    }
    function Jt(e, t, n) {
        e.bindTexture(t >>> 0, n);
    }
    function Yt(e, t) {
        e.bindVertexArrayOES(t);
    }
    function Xt(e, t) {
        e.bindVertexArray(t);
    }
    function Zt(e, t, n, r, i) {
        e.blendColor(t, n, r, i);
    }
    function Qt(e, t, n, r, i) {
        e.blendColor(t, n, r, i);
    }
    function $t(e, t, n) {
        e.blendEquationSeparate(t >>> 0, n >>> 0);
    }
    function en(e, t, n) {
        e.blendEquationSeparate(t >>> 0, n >>> 0);
    }
    function tn(e, t) {
        e.blendEquation(t >>> 0);
    }
    function nn(e, t) {
        e.blendEquation(t >>> 0);
    }
    function rn(e, t, n, r, i) {
        e.blendFuncSeparate(t >>> 0, n >>> 0, r >>> 0, i >>> 0);
    }
    function an(e, t, n, r, i) {
        e.blendFuncSeparate(t >>> 0, n >>> 0, r >>> 0, i >>> 0);
    }
    function on(e, t, n) {
        e.blendFunc(t >>> 0, n >>> 0);
    }
    function sn(e, t, n) {
        e.blendFunc(t >>> 0, n >>> 0);
    }
    function cn(e, t, n, r, i, a, o, s, c, l, u) {
        e.blitFramebuffer(t, n, r, i, a, o, s, c, l >>> 0, u >>> 0);
    }
    function ln(e, t, n, r) {
        e.bufferData(t >>> 0, n, r >>> 0);
    }
    function un(e, t, n, r) {
        e.bufferData(t >>> 0, n, r >>> 0);
    }
    function dn(e, t, n, r) {
        e.bufferData(t >>> 0, n, r >>> 0);
    }
    function fn(e, t, n, r) {
        e.bufferData(t >>> 0, n, r >>> 0);
    }
    function pn(e, t, n, r) {
        e.bufferSubData(t >>> 0, n, r);
    }
    function mn(e, t, n, r) {
        e.bufferSubData(t >>> 0, n, r);
    }
    function hn() {
        return q(function(e, t, n) {
            return e.call(t, n);
        }, arguments);
    }
    function gn(e, t, n, r, i) {
        e.clearBufferfv(t >>> 0, n, P(r, i));
    }
    function _n(e, t, n, r, i) {
        e.clearBufferiv(t >>> 0, n, F(r, i));
    }
    function vn(e, t, n, r, i) {
        e.clearBufferuiv(t >>> 0, n, I(r, i));
    }
    function yn(e, t) {
        e.clearDepth(t);
    }
    function bn(e, t) {
        e.clearDepth(t);
    }
    function xn(e, t) {
        e.clearStencil(t);
    }
    function Sn(e, t) {
        e.clearStencil(t);
    }
    function Cn(e, t) {
        e.clear(t >>> 0);
    }
    function wn(e, t) {
        e.clear(t >>> 0);
    }
    function Tn(e) {
        return e.clientHeight;
    }
    function En(e, t, n, r) {
        return e.clientWaitSync(t, n >>> 0, r >>> 0);
    }
    function Dn(e) {
        return e.clientWidth;
    }
    function On(e, t, n, r, i) {
        e.colorMask(t !== 0, n !== 0, r !== 0, i !== 0);
    }
    function kn(e, t, n, r, i) {
        e.colorMask(t !== 0, n !== 0, r !== 0, i !== 0);
    }
    function An(e, t) {
        e.compileShader(t);
    }
    function jn(e, t) {
        e.compileShader(t);
    }
    function Mn(e, t, n, r, i, a, o, s, c) {
        e.compressedTexSubImage2D(t >>> 0, n, r, i, a, o, s >>> 0, c);
    }
    function Nn(e, t, n, r, i, a, o, s, c) {
        e.compressedTexSubImage2D(t >>> 0, n, r, i, a, o, s >>> 0, c);
    }
    function Pn(e, t, n, r, i, a, o, s, c, l) {
        e.compressedTexSubImage2D(t >>> 0, n, r, i, a, o, s >>> 0, c, l);
    }
    function Fn(e, t, n, r, i, a, o, s, c, l, u, d) {
        e.compressedTexSubImage3D(t >>> 0, n, r, i, a, o, s, c, l >>> 0, u, d);
    }
    function In(e, t, n, r, i, a, o, s, c, l, u) {
        e.compressedTexSubImage3D(t >>> 0, n, r, i, a, o, s, c, l >>> 0, u);
    }
    function Ln() {
        return q(function(e, t) {
            e.configure(t);
        }, arguments);
    }
    function Rn(e, t, n, r, i, a) {
        e.copyBufferSubData(t >>> 0, n >>> 0, r, i, a);
    }
    function zn(e, t, n, r, i, a, o, s, c) {
        e.copyTexSubImage2D(t >>> 0, n, r, i, a, o, s, c);
    }
    function Bn(e, t, n, r, i, a, o, s, c) {
        e.copyTexSubImage2D(t >>> 0, n, r, i, a, o, s, c);
    }
    function Vn(e, t, n, r, i, a, o, s, c, l) {
        e.copyTexSubImage3D(t >>> 0, n, r, i, a, o, s, c, l);
    }
    function Hn() {
        return q(function(e, t) {
            return e.createBindGroupLayout(t);
        }, arguments);
    }
    function Un(e, t) {
        return e.createBindGroup(t);
    }
    function Wn(e) {
        let t = e.createBuffer();
        return J(t) ? 0 : N(t);
    }
    function Gn() {
        return q(function(e, t) {
            return e.createBuffer(t);
        }, arguments);
    }
    function Kn(e) {
        let t = e.createBuffer();
        return J(t) ? 0 : N(t);
    }
    function qn(e, t) {
        return e.createCommandEncoder(t);
    }
    function Jn(e) {
        let t = e.createFramebuffer();
        return J(t) ? 0 : N(t);
    }
    function Yn(e) {
        let t = e.createFramebuffer();
        return J(t) ? 0 : N(t);
    }
    function Xn(e, t) {
        return e.createPipelineLayout(t);
    }
    function Zn(e) {
        let t = e.createProgram();
        return J(t) ? 0 : N(t);
    }
    function Qn(e) {
        let t = e.createProgram();
        return J(t) ? 0 : N(t);
    }
    function $n(e) {
        let t = e.createQuery();
        return J(t) ? 0 : N(t);
    }
    function er() {
        return q(function(e, t) {
            return e.createRenderPipeline(t);
        }, arguments);
    }
    function tr(e) {
        let t = e.createRenderbuffer();
        return J(t) ? 0 : N(t);
    }
    function nr(e) {
        let t = e.createRenderbuffer();
        return J(t) ? 0 : N(t);
    }
    function rr(e) {
        let t = e.createSampler();
        return J(t) ? 0 : N(t);
    }
    function ir(e, t) {
        return e.createShaderModule(t);
    }
    function ar(e, t) {
        let n = e.createShader(t >>> 0);
        return J(n) ? 0 : N(n);
    }
    function or(e, t) {
        let n = e.createShader(t >>> 0);
        return J(n) ? 0 : N(n);
    }
    function sr(e) {
        let t = e.createTexture();
        return J(t) ? 0 : N(t);
    }
    function cr(e) {
        let t = e.createTexture();
        return J(t) ? 0 : N(t);
    }
    function lr(e) {
        let t = e.createVertexArrayOES();
        return J(t) ? 0 : N(t);
    }
    function ur(e) {
        let t = e.createVertexArray();
        return J(t) ? 0 : N(t);
    }
    function dr() {
        return q(function(e, t) {
            return e.createView(t);
        }, arguments);
    }
    function fr(e, t) {
        e.cullFace(t >>> 0);
    }
    function pr(e, t) {
        e.cullFace(t >>> 0);
    }
    function mr(e) {
        return vt.__wrap(e);
    }
    function hr(e, t) {
        e.deleteBuffer(t);
    }
    function gr(e, t) {
        e.deleteBuffer(t);
    }
    function _r(e, t) {
        e.deleteFramebuffer(t);
    }
    function vr(e, t) {
        e.deleteFramebuffer(t);
    }
    function yr(e, t) {
        e.deleteProgram(t);
    }
    function br(e, t) {
        e.deleteProgram(t);
    }
    function xr(e, t) {
        e.deleteQuery(t);
    }
    function Sr(e, t) {
        e.deleteRenderbuffer(t);
    }
    function Cr(e, t) {
        e.deleteRenderbuffer(t);
    }
    function wr(e, t) {
        e.deleteSampler(t);
    }
    function Tr(e, t) {
        e.deleteShader(t);
    }
    function Er(e, t) {
        e.deleteShader(t);
    }
    function Dr(e, t) {
        e.deleteSync(t);
    }
    function Or(e, t) {
        e.deleteTexture(t);
    }
    function kr(e, t) {
        e.deleteTexture(t);
    }
    function Ar(e, t) {
        e.deleteVertexArrayOES(t);
    }
    function jr(e, t) {
        e.deleteVertexArray(t);
    }
    function Mr(e, t) {
        e.depthFunc(t >>> 0);
    }
    function Nr(e, t) {
        e.depthFunc(t >>> 0);
    }
    function Pr(e, t) {
        e.depthMask(t !== 0);
    }
    function Fr(e, t) {
        e.depthMask(t !== 0);
    }
    function Ir(e, t, n) {
        e.depthRange(t, n);
    }
    function Lr(e, t, n) {
        e.depthRange(t, n);
    }
    function Rr(e) {
        e.destroy();
    }
    function zr(e, t) {
        e.disableVertexAttribArray(t >>> 0);
    }
    function Br(e, t) {
        e.disableVertexAttribArray(t >>> 0);
    }
    function Vr(e, t) {
        e.disable(t >>> 0);
    }
    function Hr(e, t) {
        e.disable(t >>> 0);
    }
    function Ur(e) {
        let t = e.document;
        return J(t) ? 0 : N(t);
    }
    function Wr(e, t, n, r, i) {
        e.drawArraysInstancedANGLE(t >>> 0, n, r, i);
    }
    function Gr(e, t, n, r, i) {
        e.drawArraysInstanced(t >>> 0, n, r, i);
    }
    function Kr(e, t, n, r) {
        e.drawArrays(t >>> 0, n, r);
    }
    function qr(e, t, n, r) {
        e.drawArrays(t >>> 0, n, r);
    }
    function Jr(e, t) {
        e.drawBuffersWEBGL(t);
    }
    function Yr(e, t) {
        e.drawBuffers(t);
    }
    function Xr(e, t, n, r, i, a) {
        e.drawElementsInstancedANGLE(t >>> 0, n, r >>> 0, i, a);
    }
    function Zr(e, t, n, r, i, a) {
        e.drawElementsInstanced(t >>> 0, n, r >>> 0, i, a);
    }
    function Qr(e, t, n, r, i) {
        e.draw(t >>> 0, n >>> 0, r >>> 0, i >>> 0);
    }
    function $r(e, t) {
        e.enableVertexAttribArray(t >>> 0);
    }
    function ei(e, t) {
        e.enableVertexAttribArray(t >>> 0);
    }
    function ti(e, t) {
        e.enable(t >>> 0);
    }
    function ni(e, t) {
        e.enable(t >>> 0);
    }
    function ri(e, t) {
        e.endQuery(t >>> 0);
    }
    function ii(e) {
        e.end();
    }
    function ai(e, t) {
        let n, r;
        try {
            n = e, r = t, console.error(U(e, t));
        } finally{
            $.__wbindgen_free(n, r, 1);
        }
    }
    function oi(e, t, n) {
        let r = e.fenceSync(t >>> 0, n >>> 0);
        return J(r) ? 0 : N(r);
    }
    function si(e, t) {
        return e.finish(t);
    }
    function ci(e) {
        return e.finish();
    }
    function li(e) {
        e.flush();
    }
    function ui(e) {
        e.flush();
    }
    function di(e, t, n, r, i) {
        e.framebufferRenderbuffer(t >>> 0, n >>> 0, r >>> 0, i);
    }
    function fi(e, t, n, r, i) {
        e.framebufferRenderbuffer(t >>> 0, n >>> 0, r >>> 0, i);
    }
    function pi(e, t, n, r, i, a) {
        e.framebufferTexture2D(t >>> 0, n >>> 0, r >>> 0, i, a);
    }
    function mi(e, t, n, r, i, a) {
        e.framebufferTexture2D(t >>> 0, n >>> 0, r >>> 0, i, a);
    }
    function hi(e, t, n, r, i, a) {
        e.framebufferTextureLayer(t >>> 0, n >>> 0, r, i, a);
    }
    function gi(e, t, n, r, i, a, o) {
        e.framebufferTextureMultiviewOVR(t >>> 0, n >>> 0, r, i, a, o);
    }
    function _i(e, t) {
        e.frontFace(t >>> 0);
    }
    function vi(e, t) {
        e.frontFace(t >>> 0);
    }
    function yi(e, t, n, r) {
        e.getBufferSubData(t >>> 0, n, r);
    }
    function bi() {
        return q(function(e, t, n, r) {
            let i = e.getContext(U(t, n), r);
            return J(i) ? 0 : N(i);
        }, arguments);
    }
    function xi() {
        return q(function(e, t, n) {
            let r = e.getContext(U(t, n));
            return J(r) ? 0 : N(r);
        }, arguments);
    }
    function Si() {
        return q(function(e, t, n) {
            let r = e.getContext(U(t, n));
            return J(r) ? 0 : N(r);
        }, arguments);
    }
    function Ci() {
        return q(function(e, t, n, r) {
            let i = e.getContext(U(t, n), r);
            return J(i) ? 0 : N(i);
        }, arguments);
    }
    function wi() {
        return q(function(e) {
            return e.getCurrentTexture();
        }, arguments);
    }
    function Ti(e, t, n) {
        let r = e.getElementById(U(t, n));
        return J(r) ? 0 : N(r);
    }
    function Ei() {
        return q(function(e, t, n) {
            let r = e.getExtension(U(t, n));
            return J(r) ? 0 : N(r);
        }, arguments);
    }
    function Di() {
        return q(function(e, t, n) {
            return e.getIndexedParameter(t >>> 0, n >>> 0);
        }, arguments);
    }
    function Oi() {
        return q(function(e, t, n) {
            return e.getMappedRange(t, n);
        }, arguments);
    }
    function ki() {
        return q(function(e, t) {
            return e.getParameter(t >>> 0);
        }, arguments);
    }
    function Ai() {
        return q(function(e, t) {
            return e.getParameter(t >>> 0);
        }, arguments);
    }
    function ji(e) {
        let t = e.getPreferredCanvasFormat();
        return (M.indexOf(t) + 1 || 96) - 1;
    }
    function Mi(e, t, n) {
        let r = t.getProgramInfoLog(n);
        var i = J(r) ? 0 : Y(r, $.__wbindgen_malloc, $.__wbindgen_realloc), a = Q;
        R().setInt32(e + 4, a, !0), R().setInt32(e + 0, i, !0);
    }
    function Ni(e, t, n) {
        let r = t.getProgramInfoLog(n);
        var i = J(r) ? 0 : Y(r, $.__wbindgen_malloc, $.__wbindgen_realloc), a = Q;
        R().setInt32(e + 4, a, !0), R().setInt32(e + 0, i, !0);
    }
    function Pi(e, t, n) {
        return e.getProgramParameter(t, n >>> 0);
    }
    function Fi(e, t, n) {
        return e.getProgramParameter(t, n >>> 0);
    }
    function Ii(e, t, n) {
        return e.getQueryParameter(t, n >>> 0);
    }
    function Li(e, t, n) {
        let r = t.getShaderInfoLog(n);
        var i = J(r) ? 0 : Y(r, $.__wbindgen_malloc, $.__wbindgen_realloc), a = Q;
        R().setInt32(e + 4, a, !0), R().setInt32(e + 0, i, !0);
    }
    function Ri(e, t, n) {
        let r = t.getShaderInfoLog(n);
        var i = J(r) ? 0 : Y(r, $.__wbindgen_malloc, $.__wbindgen_realloc), a = Q;
        R().setInt32(e + 4, a, !0), R().setInt32(e + 0, i, !0);
    }
    function zi(e, t, n) {
        return e.getShaderParameter(t, n >>> 0);
    }
    function Bi(e, t, n) {
        return e.getShaderParameter(t, n >>> 0);
    }
    function Vi(e) {
        let t = e.getSupportedExtensions();
        return J(t) ? 0 : N(t);
    }
    function Hi(e) {
        let t = e.getSupportedProfiles();
        return J(t) ? 0 : N(t);
    }
    function Ui(e, t, n) {
        return e.getSyncParameter(t, n >>> 0);
    }
    function Wi(e, t, n, r) {
        return e.getUniformBlockIndex(t, U(n, r));
    }
    function Gi(e, t, n, r) {
        let i = e.getUniformLocation(t, U(n, r));
        return J(i) ? 0 : N(i);
    }
    function Ki(e, t, n, r) {
        let i = e.getUniformLocation(t, U(n, r));
        return J(i) ? 0 : N(i);
    }
    function qi(e, t) {
        let n = e[t >>> 0];
        return J(n) ? 0 : N(n);
    }
    function Ji(e, t) {
        return e[t >>> 0];
    }
    function Yi(e) {
        return e.gpu;
    }
    function Xi(e) {
        return e.height;
    }
    function Zi(e, t, n) {
        return e.includes(t, n);
    }
    function Qi(e) {
        let t;
        try {
            t = e instanceof GPUAdapter;
        } catch  {
            t = !1;
        }
        return t;
    }
    function $i(e) {
        let t;
        try {
            t = e instanceof GPUCanvasContext;
        } catch  {
            t = !1;
        }
        return t;
    }
    function ea(e) {
        let t;
        try {
            t = e instanceof HTMLCanvasElement;
        } catch  {
            t = !1;
        }
        return t;
    }
    function ta(e) {
        let t;
        try {
            t = e instanceof WebGL2RenderingContext;
        } catch  {
            t = !1;
        }
        return t;
    }
    function na(e) {
        let t;
        try {
            t = e instanceof Window;
        } catch  {
            t = !1;
        }
        return t;
    }
    function ra() {
        return q(function(e, t, n) {
            e.invalidateFramebuffer(t >>> 0, n);
        }, arguments);
    }
    function ia(e, t) {
        return Object.is(e, t);
    }
    function aa(e, t) {
        let n = t.label, r = Y(n, $.__wbindgen_malloc, $.__wbindgen_realloc), i = Q;
        R().setInt32(e + 4, i, !0), R().setInt32(e + 0, r, !0);
    }
    function oa(e) {
        return e.length;
    }
    function sa(e, t) {
        e.linkProgram(t);
    }
    function ca(e, t) {
        e.linkProgram(t);
    }
    function la(e, t) {
        console.log(U(e, t));
    }
    function ua(e, t, n, r) {
        return e.mapAsync(t >>> 0, n, r);
    }
    function da(e) {
        return e.navigator;
    }
    function fa(e) {
        return e.navigator;
    }
    function pa() {
        return Error();
    }
    function ma() {
        return {};
    }
    function ha() {
        return [];
    }
    function ga(e, t) {
        try {
            var n = {
                a: e,
                b: t
            };
            return new Promise((e, t)=>{
                let r = n.a;
                n.a = 0;
                try {
                    return xu(r, n.b, e, t);
                } finally{
                    n.a = r;
                }
            });
        } finally{
            n.a = 0;
        }
    }
    function _a(e, t, n) {
        return new Uint8Array(e, t >>> 0, n >>> 0);
    }
    function va(e) {
        return e.now();
    }
    function ya(e) {
        return [
            e
        ];
    }
    function ba(e) {
        return e.onSubmittedWorkDone();
    }
    function xa(e) {
        let t = e.performance;
        return J(t) ? 0 : N(t);
    }
    function Sa(e, t, n) {
        e.pixelStorei(t >>> 0, n);
    }
    function Ca(e, t, n) {
        e.pixelStorei(t >>> 0, n);
    }
    function wa(e, t, n) {
        e.polygonOffset(t, n);
    }
    function Ta(e, t, n) {
        e.polygonOffset(t, n);
    }
    function Ea(e, t) {
        return e.push(t);
    }
    function Da(e, t, n) {
        e.queryCounterEXT(t, n >>> 0);
    }
    function Oa() {
        return q(function(e, t, n) {
            return e.querySelectorAll(U(t, n));
        }, arguments);
    }
    function ka() {
        return q(function(e, t, n) {
            let r = e.querySelector(U(t, n));
            return J(r) ? 0 : N(r);
        }, arguments);
    }
    function Aa(e) {
        return e.queueMicrotask;
    }
    function ja(e) {
        queueMicrotask(e);
    }
    function Ma(e) {
        return e.queue;
    }
    function Na(e, t) {
        e.readBuffer(t >>> 0);
    }
    function Pa() {
        return q(function(e, t, n, r, i, a, o, s) {
            e.readPixels(t, n, r, i, a >>> 0, o >>> 0, s);
        }, arguments);
    }
    function Fa() {
        return q(function(e, t, n, r, i, a, o, s) {
            e.readPixels(t, n, r, i, a >>> 0, o >>> 0, s);
        }, arguments);
    }
    function Ia() {
        return q(function(e, t, n, r, i, a, o, s) {
            e.readPixels(t, n, r, i, a >>> 0, o >>> 0, s);
        }, arguments);
    }
    function La(e, t, n, r, i, a) {
        e.renderbufferStorageMultisample(t >>> 0, n, r >>> 0, i, a);
    }
    function Ra(e, t, n, r, i) {
        e.renderbufferStorage(t >>> 0, n >>> 0, r, i);
    }
    function za(e, t, n, r, i) {
        e.renderbufferStorage(t >>> 0, n >>> 0, r, i);
    }
    function Ba(e, t) {
        return e.requestAdapter(t);
    }
    function Va() {
        return q(function(e, t) {
            return e.requestAnimationFrame(t);
        }, arguments);
    }
    function Ha(e, t) {
        return e.requestDevice(t);
    }
    function Ua(e) {
        return Promise.resolve(e);
    }
    function Wa(e, t, n, r) {
        e.samplerParameterf(t, n >>> 0, r);
    }
    function Ga(e, t, n, r) {
        e.samplerParameteri(t, n >>> 0, r);
    }
    function Ka(e, t, n, r, i) {
        e.scissor(t, n, r, i);
    }
    function qa(e, t, n, r, i) {
        e.scissor(t, n, r, i);
    }
    function Ja() {
        return q(function(e, t, n, r, i, a, o) {
            e.setBindGroup(t >>> 0, n, I(r, i), a, o >>> 0);
        }, arguments);
    }
    function Ya(e, t, n) {
        e.setBindGroup(t >>> 0, n);
    }
    function Xa(e, t) {
        e.setPipeline(t);
    }
    function Za() {
        return q(function(e, t, n) {
            return Reflect.set(e, t, n);
        }, arguments);
    }
    function Qa(e, t) {
        e.a = t;
    }
    function $a(e, t) {
        e.access = Iu[t];
    }
    function eo(e, t) {
        e.alpha = t;
    }
    function to(e, t) {
        e.alphaMode = Eu[t];
    }
    function no(e, t) {
        e.alphaToCoverageEnabled = t !== 0;
    }
    function ro(e, t) {
        e.arrayLayerCount = t >>> 0;
    }
    function io(e, t) {
        e.arrayStride = t;
    }
    function ao(e, t) {
        e.aspect = Ru[t];
    }
    function oo(e, t) {
        e.attributes = t;
    }
    function so(e, t) {
        e.b = t;
    }
    function co(e, t) {
        e.baseArrayLayer = t >>> 0;
    }
    function lo(e, t) {
        e.baseMipLevel = t >>> 0;
    }
    function uo(e, t) {
        e.beginningOfPassWriteIndex = t >>> 0;
    }
    function fo(e, t) {
        e.bindGroupLayouts = t;
    }
    function po(e, t) {
        e.binding = t >>> 0;
    }
    function mo(e, t) {
        e.binding = t >>> 0;
    }
    function ho(e, t) {
        e.blend = t;
    }
    function go(e, t) {
        e.buffer = t;
    }
    function _o(e, t) {
        e.buffer = t;
    }
    function vo(e, t) {
        e.buffers = t;
    }
    function yo(e, t) {
        e.clearValue = t;
    }
    function bo(e, t, n) {
        e.code = U(t, n);
    }
    function xo(e, t) {
        e.color = t;
    }
    function So(e, t) {
        e.colorAttachments = t;
    }
    function Co(e, t) {
        e.compare = Du[t];
    }
    function wo(e, t) {
        e.count = t >>> 0;
    }
    function To(e, t) {
        e.cullMode = Ou[t];
    }
    function Eo(e, t) {
        e.depthBias = t;
    }
    function Do(e, t) {
        e.depthBiasClamp = t;
    }
    function Oo(e, t) {
        e.depthBiasSlopeScale = t;
    }
    function ko(e, t) {
        e.depthClearValue = t;
    }
    function Ao(e, t) {
        e.depthCompare = Du[t];
    }
    function jo(e, t) {
        e.depthFailOp = Fu[t];
    }
    function Mo(e, t) {
        e.depthLoadOp = ju[t];
    }
    function No(e, t) {
        e.depthReadOnly = t !== 0;
    }
    function Po(e, t) {
        e.depthStencilAttachment = t;
    }
    function Fo(e, t) {
        e.depthStencil = t;
    }
    function Io(e, t) {
        e.depthStoreOp = Lu[t];
    }
    function Lo(e, t) {
        e.depthWriteEnabled = t !== 0;
    }
    function Ro(e, t) {
        e.device = t;
    }
    function zo(e, t) {
        e.dimension = Bu[t];
    }
    function Bo(e, t) {
        e.dstFactor = Cu[t];
    }
    function Vo(e, t) {
        e.endOfPassWriteIndex = t >>> 0;
    }
    function Ho(e, t) {
        e.entries = t;
    }
    function Uo(e, t) {
        e.entries = t;
    }
    function Wo(e, t, n) {
        e.entryPoint = U(t, n);
    }
    function Go(e, t, n) {
        e.entryPoint = U(t, n);
    }
    function Ko(e, t) {
        e.externalTexture = t;
    }
    function qo(e, t) {
        e.failOp = Fu[t];
    }
    function Jo(e, t) {
        e.format = M[t];
    }
    function Yo(e, t) {
        e.format = M[t];
    }
    function Xo(e, t) {
        e.format = M[t];
    }
    function Zo(e, t) {
        e.format = Vu[t];
    }
    function Qo(e, t) {
        e.format = M[t];
    }
    function $o(e, t) {
        e.format = M[t];
    }
    function es(e, t) {
        e.fragment = t;
    }
    function ts(e, t) {
        e.frontFace = ku[t];
    }
    function ns(e, t) {
        e.g = t;
    }
    function rs(e, t) {
        e.hasDynamicOffset = t !== 0;
    }
    function is(e, t) {
        e.height = t >>> 0;
    }
    function as(e, t) {
        e.height = t >>> 0;
    }
    function os(e, t, n) {
        e.label = U(t, n);
    }
    function ss(e, t, n) {
        e.label = U(t, n);
    }
    function cs(e, t, n) {
        e.label = U(t, n);
    }
    function ls(e, t, n) {
        e.label = U(t, n);
    }
    function us(e, t, n) {
        e.label = U(t, n);
    }
    function ds(e, t, n) {
        e.label = U(t, n);
    }
    function fs(e, t, n) {
        e.label = U(t, n);
    }
    function ps(e, t, n) {
        e.label = U(t, n);
    }
    function ms(e, t, n) {
        e.label = U(t, n);
    }
    function hs(e, t, n) {
        e.label = U(t, n);
    }
    function gs(e, t, n) {
        e.label = U(t, n);
    }
    function _s(e, t) {
        e.layout = t;
    }
    function vs(e, t) {
        e.layout = t;
    }
    function ys(e, t) {
        e.loadOp = ju[t];
    }
    function bs(e, t) {
        e.mappedAtCreation = t !== 0;
    }
    function xs(e, t) {
        e.mask = t >>> 0;
    }
    function Ss(e, t) {
        e.minBindingSize = t;
    }
    function Cs(e, t) {
        e.mipLevelCount = t >>> 0;
    }
    function ws(e, t) {
        e.module = t;
    }
    function Ts(e, t) {
        e.module = t;
    }
    function Es(e, t) {
        e.multisample = t;
    }
    function Ds(e, t) {
        e.multisampled = t !== 0;
    }
    function Os(e, t) {
        e.offset = t;
    }
    function ks(e, t) {
        e.offset = t;
    }
    function As(e, t) {
        e.operation = wu[t];
    }
    function js(e, t) {
        e.passOp = Fu[t];
    }
    function Ms(e, t) {
        e.powerPreference = Mu[t];
    }
    function Ns(e, t) {
        e.primitive = t;
    }
    function Ps(e, t) {
        e.querySet = t;
    }
    function Fs(e, t) {
        e.r = t;
    }
    function Is(e, t) {
        e.requiredFeatures = t;
    }
    function Ls(e, t) {
        e.requiredLimits = t;
    }
    function Rs(e, t) {
        e.resolveTarget = t;
    }
    function zs(e, t) {
        e.resource = t;
    }
    function Bs(e, t) {
        e.sampleType = zu[t];
    }
    function Vs(e, t) {
        e.sampler = t;
    }
    function Hs(e, t) {
        e.shaderLocation = t >>> 0;
    }
    function Us(e, t) {
        e.size = t;
    }
    function Ws(e, t) {
        e.size = t;
    }
    function Gs(e, t) {
        e.srcFactor = Cu[t];
    }
    function Ks(e, t) {
        e.stencilBack = t;
    }
    function qs(e, t) {
        e.stencilClearValue = t >>> 0;
    }
    function Js(e, t) {
        e.stencilFront = t;
    }
    function Ys(e, t) {
        e.stencilLoadOp = ju[t];
    }
    function Xs(e, t) {
        e.stencilReadMask = t >>> 0;
    }
    function Zs(e, t) {
        e.stencilReadOnly = t !== 0;
    }
    function Qs(e, t) {
        e.stencilStoreOp = Lu[t];
    }
    function $s(e, t) {
        e.stencilWriteMask = t >>> 0;
    }
    function ec(e, t) {
        e.stepMode = Hu[t];
    }
    function tc(e, t) {
        e.storageTexture = t;
    }
    function nc(e, t) {
        e.storeOp = Lu[t];
    }
    function rc(e, t) {
        e.stripIndexFormat = Au[t];
    }
    function ic(e, t) {
        e.targets = t;
    }
    function ac(e, t) {
        e.texture = t;
    }
    function oc(e, t) {
        e.timestampWrites = t;
    }
    function sc(e, t) {
        e.topology = Nu[t];
    }
    function cc(e, t) {
        e.type = Tu[t];
    }
    function lc(e, t) {
        e.type = Pu[t];
    }
    function uc(e, t) {
        e.unclippedDepth = t !== 0;
    }
    function dc(e, t) {
        e.usage = t >>> 0;
    }
    function fc(e, t) {
        e.usage = t >>> 0;
    }
    function pc(e, t) {
        e.usage = t >>> 0;
    }
    function mc(e, t) {
        e.vertex = t;
    }
    function hc(e, t) {
        e.view = t;
    }
    function gc(e, t) {
        e.view = t;
    }
    function _c(e, t) {
        e.viewDimension = Bu[t];
    }
    function vc(e, t) {
        e.viewDimension = Bu[t];
    }
    function yc(e, t) {
        e.viewFormats = t;
    }
    function bc(e, t) {
        e.visibility = t >>> 0;
    }
    function xc(e, t) {
        e.width = t >>> 0;
    }
    function Sc(e, t) {
        e.width = t >>> 0;
    }
    function Cc(e, t) {
        e.writeMask = t >>> 0;
    }
    function wc(e, t, n, r) {
        e.shaderSource(t, U(n, r));
    }
    function Tc(e, t, n, r) {
        e.shaderSource(t, U(n, r));
    }
    function Ec(e, t) {
        let n = t.stack, r = Y(n, $.__wbindgen_malloc, $.__wbindgen_realloc), i = Q;
        R().setInt32(e + 4, i, !0), R().setInt32(e + 0, r, !0);
    }
    function Dc() {
        let e = typeof globalThis > `u` ? null : globalThis;
        return J(e) ? 0 : N(e);
    }
    function Oc() {
        let e = typeof global > `u` ? null : global;
        return J(e) ? 0 : N(e);
    }
    function kc() {
        let e = typeof self > `u` ? null : self;
        return J(e) ? 0 : N(e);
    }
    function Ac() {
        let e = typeof window > `u` ? null : window;
        return J(e) ? 0 : N(e);
    }
    function jc(e, t, n, r, i) {
        e.stencilFuncSeparate(t >>> 0, n >>> 0, r, i >>> 0);
    }
    function Mc(e, t, n, r, i) {
        e.stencilFuncSeparate(t >>> 0, n >>> 0, r, i >>> 0);
    }
    function Nc(e, t, n) {
        e.stencilMaskSeparate(t >>> 0, n >>> 0);
    }
    function Pc(e, t, n) {
        e.stencilMaskSeparate(t >>> 0, n >>> 0);
    }
    function Fc(e, t) {
        e.stencilMask(t >>> 0);
    }
    function Ic(e, t) {
        e.stencilMask(t >>> 0);
    }
    function Lc(e, t, n, r, i) {
        e.stencilOpSeparate(t >>> 0, n >>> 0, r >>> 0, i >>> 0);
    }
    function Rc(e, t, n, r, i) {
        e.stencilOpSeparate(t >>> 0, n >>> 0, r >>> 0, i >>> 0);
    }
    function zc(e, t) {
        e.submit(t);
    }
    function Bc() {
        return q(function(e, t, n, r, i, a, o, s, c, l) {
            e.texImage2D(t >>> 0, n, r, i, a, o, s >>> 0, c >>> 0, l);
        }, arguments);
    }
    function Vc() {
        return q(function(e, t, n, r, i, a, o, s, c, l) {
            e.texImage2D(t >>> 0, n, r, i, a, o, s >>> 0, c >>> 0, l);
        }, arguments);
    }
    function Hc() {
        return q(function(e, t, n, r, i, a, o, s, c, l) {
            e.texImage2D(t >>> 0, n, r, i, a, o, s >>> 0, c >>> 0, l);
        }, arguments);
    }
    function Uc() {
        return q(function(e, t, n, r, i, a, o, s, c, l, u) {
            e.texImage3D(t >>> 0, n, r, i, a, o, s, c >>> 0, l >>> 0, u);
        }, arguments);
    }
    function Wc() {
        return q(function(e, t, n, r, i, a, o, s, c, l, u) {
            e.texImage3D(t >>> 0, n, r, i, a, o, s, c >>> 0, l >>> 0, u);
        }, arguments);
    }
    function Gc(e, t, n, r) {
        e.texParameteri(t >>> 0, n >>> 0, r);
    }
    function Kc(e, t, n, r) {
        e.texParameteri(t >>> 0, n >>> 0, r);
    }
    function qc(e, t, n, r, i, a) {
        e.texStorage2D(t >>> 0, n, r >>> 0, i, a);
    }
    function Jc(e, t, n, r, i, a, o) {
        e.texStorage3D(t >>> 0, n, r >>> 0, i, a, o);
    }
    function Yc() {
        return q(function(e, t, n, r, i, a, o, s, c, l) {
            e.texSubImage2D(t >>> 0, n, r, i, a, o, s >>> 0, c >>> 0, l);
        }, arguments);
    }
    function Xc() {
        return q(function(e, t, n, r, i, a, o, s, c, l) {
            e.texSubImage2D(t >>> 0, n, r, i, a, o, s >>> 0, c >>> 0, l);
        }, arguments);
    }
    function Zc() {
        return q(function(e, t, n, r, i, a, o, s, c, l) {
            e.texSubImage2D(t >>> 0, n, r, i, a, o, s >>> 0, c >>> 0, l);
        }, arguments);
    }
    function Qc() {
        return q(function(e, t, n, r, i, a, o, s, c, l) {
            e.texSubImage2D(t >>> 0, n, r, i, a, o, s >>> 0, c >>> 0, l);
        }, arguments);
    }
    function $c() {
        return q(function(e, t, n, r, i, a, o, s, c, l) {
            e.texSubImage2D(t >>> 0, n, r, i, a, o, s >>> 0, c >>> 0, l);
        }, arguments);
    }
    function el() {
        return q(function(e, t, n, r, i, a, o, s, c, l) {
            e.texSubImage2D(t >>> 0, n, r, i, a, o, s >>> 0, c >>> 0, l);
        }, arguments);
    }
    function tl() {
        return q(function(e, t, n, r, i, a, o, s, c, l) {
            e.texSubImage2D(t >>> 0, n, r, i, a, o, s >>> 0, c >>> 0, l);
        }, arguments);
    }
    function nl() {
        return q(function(e, t, n, r, i, a, o, s, c, l) {
            e.texSubImage2D(t >>> 0, n, r, i, a, o, s >>> 0, c >>> 0, l);
        }, arguments);
    }
    function rl() {
        return q(function(e, t, n, r, i, a, o, s, c, l, u, d) {
            e.texSubImage3D(t >>> 0, n, r, i, a, o, s, c, l >>> 0, u >>> 0, d);
        }, arguments);
    }
    function il() {
        return q(function(e, t, n, r, i, a, o, s, c, l, u, d) {
            e.texSubImage3D(t >>> 0, n, r, i, a, o, s, c, l >>> 0, u >>> 0, d);
        }, arguments);
    }
    function al() {
        return q(function(e, t, n, r, i, a, o, s, c, l, u, d) {
            e.texSubImage3D(t >>> 0, n, r, i, a, o, s, c, l >>> 0, u >>> 0, d);
        }, arguments);
    }
    function ol() {
        return q(function(e, t, n, r, i, a, o, s, c, l, u, d) {
            e.texSubImage3D(t >>> 0, n, r, i, a, o, s, c, l >>> 0, u >>> 0, d);
        }, arguments);
    }
    function sl() {
        return q(function(e, t, n, r, i, a, o, s, c, l, u, d) {
            e.texSubImage3D(t >>> 0, n, r, i, a, o, s, c, l >>> 0, u >>> 0, d);
        }, arguments);
    }
    function cl() {
        return q(function(e, t, n, r, i, a, o, s, c, l, u, d) {
            e.texSubImage3D(t >>> 0, n, r, i, a, o, s, c, l >>> 0, u >>> 0, d);
        }, arguments);
    }
    function ll() {
        return q(function(e, t, n, r, i, a, o, s, c, l, u, d) {
            e.texSubImage3D(t >>> 0, n, r, i, a, o, s, c, l >>> 0, u >>> 0, d);
        }, arguments);
    }
    function ul(e, t, n) {
        return e.then(t, n);
    }
    function dl(e, t, n) {
        return e.then(t, n);
    }
    function fl(e, t) {
        return e.then(t);
    }
    function pl(e, t, n) {
        e.uniform1f(t, n);
    }
    function ml(e, t, n) {
        e.uniform1f(t, n);
    }
    function hl(e, t, n) {
        e.uniform1i(t, n);
    }
    function gl(e, t, n) {
        e.uniform1i(t, n);
    }
    function _l(e, t, n) {
        e.uniform1ui(t, n >>> 0);
    }
    function vl(e, t, n, r) {
        e.uniform2fv(t, P(n, r));
    }
    function yl(e, t, n, r) {
        e.uniform2fv(t, P(n, r));
    }
    function bl(e, t, n, r) {
        e.uniform2iv(t, F(n, r));
    }
    function xl(e, t, n, r) {
        e.uniform2iv(t, F(n, r));
    }
    function Sl(e, t, n, r) {
        e.uniform2uiv(t, I(n, r));
    }
    function Cl(e, t, n, r) {
        e.uniform3fv(t, P(n, r));
    }
    function wl(e, t, n, r) {
        e.uniform3fv(t, P(n, r));
    }
    function Tl(e, t, n, r) {
        e.uniform3iv(t, F(n, r));
    }
    function El(e, t, n, r) {
        e.uniform3iv(t, F(n, r));
    }
    function Dl(e, t, n, r) {
        e.uniform3uiv(t, I(n, r));
    }
    function Ol(e, t, n, r, i, a) {
        e.uniform4f(t, n, r, i, a);
    }
    function kl(e, t, n, r, i, a) {
        e.uniform4f(t, n, r, i, a);
    }
    function Al(e, t, n, r) {
        e.uniform4fv(t, P(n, r));
    }
    function jl(e, t, n, r) {
        e.uniform4fv(t, P(n, r));
    }
    function Ml(e, t, n, r) {
        e.uniform4iv(t, F(n, r));
    }
    function Nl(e, t, n, r) {
        e.uniform4iv(t, F(n, r));
    }
    function Pl(e, t, n, r) {
        e.uniform4uiv(t, I(n, r));
    }
    function Fl(e, t, n, r) {
        e.uniformBlockBinding(t, n >>> 0, r >>> 0);
    }
    function Il(e, t, n, r, i) {
        e.uniformMatrix2fv(t, n !== 0, P(r, i));
    }
    function Ll(e, t, n, r, i) {
        e.uniformMatrix2fv(t, n !== 0, P(r, i));
    }
    function Rl(e, t, n, r, i) {
        e.uniformMatrix2x3fv(t, n !== 0, P(r, i));
    }
    function zl(e, t, n, r, i) {
        e.uniformMatrix2x4fv(t, n !== 0, P(r, i));
    }
    function Bl(e, t, n, r, i) {
        e.uniformMatrix3fv(t, n !== 0, P(r, i));
    }
    function Vl(e, t, n, r, i) {
        e.uniformMatrix3fv(t, n !== 0, P(r, i));
    }
    function Hl(e, t, n, r, i) {
        e.uniformMatrix3x2fv(t, n !== 0, P(r, i));
    }
    function Ul(e, t, n, r, i) {
        e.uniformMatrix3x4fv(t, n !== 0, P(r, i));
    }
    function Wl(e, t, n, r, i) {
        e.uniformMatrix4fv(t, n !== 0, P(r, i));
    }
    function Gl(e, t, n, r, i) {
        e.uniformMatrix4fv(t, n !== 0, P(r, i));
    }
    function Kl(e, t, n, r, i) {
        e.uniformMatrix4x2fv(t, n !== 0, P(r, i));
    }
    function ql(e, t, n, r, i) {
        e.uniformMatrix4x3fv(t, n !== 0, P(r, i));
    }
    function Jl(e) {
        e.unmap();
    }
    function Yl(e, t) {
        e.useProgram(t);
    }
    function Xl(e, t) {
        e.useProgram(t);
    }
    function Zl(e, t, n) {
        e.vertexAttribDivisorANGLE(t >>> 0, n >>> 0);
    }
    function Ql(e, t, n) {
        e.vertexAttribDivisor(t >>> 0, n >>> 0);
    }
    function $l(e, t, n, r, i, a) {
        e.vertexAttribIPointer(t >>> 0, n, r >>> 0, i, a);
    }
    function eu(e, t, n, r, i, a, o) {
        e.vertexAttribPointer(t >>> 0, n, r >>> 0, i !== 0, a, o);
    }
    function tu(e, t, n, r, i, a, o) {
        e.vertexAttribPointer(t >>> 0, n, r >>> 0, i !== 0, a, o);
    }
    function nu(e, t, n, r, i) {
        e.viewport(t, n, r, i);
    }
    function ru(e, t, n, r, i) {
        e.viewport(t, n, r, i);
    }
    function iu(e) {
        return e.width;
    }
    function au() {
        return q(function(e, t, n, r, i, a, o) {
            e.writeBuffer(t, n, Yu(r, i), a, o);
        }, arguments);
    }
    function ou(e, t) {
        return rd(e, t, bu);
    }
    function su(e, t) {
        return rd(e, t, yu);
    }
    function cu(e, t) {
        return rd(e, t, Su);
    }
    function lu(e) {
        return e;
    }
    function uu(e, t) {
        return P(e, t);
    }
    function du(e, t) {
        return Ku(e, t);
    }
    function fu(e, t) {
        return F(e, t);
    }
    function pu(e, t) {
        return qu(e, t);
    }
    function mu(e, t) {
        return Ju(e, t);
    }
    function hu(e, t) {
        return I(e, t);
    }
    function gu(e, t) {
        return Yu(e, t);
    }
    function _u(e, t) {
        return U(e, t);
    }
    function vu() {
        let e = $.__wbindgen_externrefs, t = e.grow(4);
        e.set(0, void 0), e.set(t + 0, void 0), e.set(t + 1, null), e.set(t + 2, !0), e.set(t + 3, !1);
    }
    function yu(e, t, n) {
        $.wasm_bindgen__convert__closures_____invoke__h951a2592665c9e68(e, t, n);
    }
    function bu(e, t, n) {
        let r = $.wasm_bindgen__convert__closures_____invoke__h1d8a7b17cad35d54(e, t, n);
        if (r[1]) throw id(r[0]);
    }
    function xu(e, t, n, r) {
        $.wasm_bindgen__convert__closures_____invoke__h602f04d6d0a72043(e, t, n, r);
    }
    function Su(e, t, n) {
        $.wasm_bindgen__convert__closures_____invoke__h5ed47cf580eaf96c(e, t, n);
    }
    var Cu = [
        `zero`,
        `one`,
        `src`,
        `one-minus-src`,
        `src-alpha`,
        `one-minus-src-alpha`,
        `dst`,
        `one-minus-dst`,
        `dst-alpha`,
        `one-minus-dst-alpha`,
        `src-alpha-saturated`,
        `constant`,
        `one-minus-constant`,
        `src1`,
        `one-minus-src1`,
        `src1-alpha`,
        `one-minus-src1-alpha`
    ], wu = [
        `add`,
        `subtract`,
        `reverse-subtract`,
        `min`,
        `max`
    ], Tu = [
        `uniform`,
        `storage`,
        `read-only-storage`
    ], Eu = [
        `opaque`,
        `premultiplied`
    ], Du = [
        `never`,
        `less`,
        `equal`,
        `less-equal`,
        `greater`,
        `not-equal`,
        `greater-equal`,
        `always`
    ], Ou = [
        `none`,
        `front`,
        `back`
    ], ku = [
        `ccw`,
        `cw`
    ], Au = [
        `uint16`,
        `uint32`
    ], ju = [
        `load`,
        `clear`
    ], Mu = [
        `low-power`,
        `high-performance`
    ], Nu = [
        `point-list`,
        `line-list`,
        `line-strip`,
        `triangle-list`,
        `triangle-strip`
    ], Pu = [
        `filtering`,
        `non-filtering`,
        `comparison`
    ], Fu = [
        `keep`,
        `zero`,
        `replace`,
        `invert`,
        `increment-clamp`,
        `decrement-clamp`,
        `increment-wrap`,
        `decrement-wrap`
    ], Iu = [
        `write-only`,
        `read-only`,
        `read-write`
    ], Lu = [
        `store`,
        `discard`
    ], Ru = [
        `all`,
        `stencil-only`,
        `depth-only`
    ], M = `r8unorm.r8snorm.r8uint.r8sint.r16uint.r16sint.r16float.rg8unorm.rg8snorm.rg8uint.rg8sint.r32uint.r32sint.r32float.rg16uint.rg16sint.rg16float.rgba8unorm.rgba8unorm-srgb.rgba8snorm.rgba8uint.rgba8sint.bgra8unorm.bgra8unorm-srgb.rgb9e5ufloat.rgb10a2uint.rgb10a2unorm.rg11b10ufloat.rg32uint.rg32sint.rg32float.rgba16uint.rgba16sint.rgba16float.rgba32uint.rgba32sint.rgba32float.stencil8.depth16unorm.depth24plus.depth24plus-stencil8.depth32float.depth32float-stencil8.bc1-rgba-unorm.bc1-rgba-unorm-srgb.bc2-rgba-unorm.bc2-rgba-unorm-srgb.bc3-rgba-unorm.bc3-rgba-unorm-srgb.bc4-r-unorm.bc4-r-snorm.bc5-rg-unorm.bc5-rg-snorm.bc6h-rgb-ufloat.bc6h-rgb-float.bc7-rgba-unorm.bc7-rgba-unorm-srgb.etc2-rgb8unorm.etc2-rgb8unorm-srgb.etc2-rgb8a1unorm.etc2-rgb8a1unorm-srgb.etc2-rgba8unorm.etc2-rgba8unorm-srgb.eac-r11unorm.eac-r11snorm.eac-rg11unorm.eac-rg11snorm.astc-4x4-unorm.astc-4x4-unorm-srgb.astc-5x4-unorm.astc-5x4-unorm-srgb.astc-5x5-unorm.astc-5x5-unorm-srgb.astc-6x5-unorm.astc-6x5-unorm-srgb.astc-6x6-unorm.astc-6x6-unorm-srgb.astc-8x5-unorm.astc-8x5-unorm-srgb.astc-8x6-unorm.astc-8x6-unorm-srgb.astc-8x8-unorm.astc-8x8-unorm-srgb.astc-10x5-unorm.astc-10x5-unorm-srgb.astc-10x6-unorm.astc-10x6-unorm-srgb.astc-10x8-unorm.astc-10x8-unorm-srgb.astc-10x10-unorm.astc-10x10-unorm-srgb.astc-12x10-unorm.astc-12x10-unorm-srgb.astc-12x12-unorm.astc-12x12-unorm-srgb`.split(`.`), zu = [
        `float`,
        `unfilterable-float`,
        `depth`,
        `sint`,
        `uint`
    ], Bu = [
        `1d`,
        `2d`,
        `2d-array`,
        `cube`,
        `cube-array`,
        `3d`
    ], Vu = `uint8.uint8x2.uint8x4.sint8.sint8x2.sint8x4.unorm8.unorm8x2.unorm8x4.snorm8.snorm8x2.snorm8x4.uint16.uint16x2.uint16x4.sint16.sint16x2.sint16x4.unorm16.unorm16x2.unorm16x4.snorm16.snorm16x2.snorm16x4.float16.float16x2.float16x4.float32.float32x2.float32x3.float32x4.uint32.uint32x2.uint32x3.uint32x4.sint32.sint32x2.sint32x3.sint32x4.unorm10-10-10-2.unorm8x4-bgra`.split(`.`), Hu = [
        `vertex`,
        `instance`
    ], Uu = typeof FinalizationRegistry > `u` ? {
        register: ()=>{},
        unregister: ()=>{}
    } : new FinalizationRegistry((e)=>$.__wbg_dashboard_free(e, 1));
    function N(e) {
        let t = $.__externref_table_alloc();
        return $.__wbindgen_externrefs.set(t, e), t;
    }
    var Wu = typeof FinalizationRegistry > `u` ? {
        register: ()=>{},
        unregister: ()=>{}
    } : new FinalizationRegistry((e)=>$.__wbindgen_destroy_closure(e.a, e.b));
    function Gu(e) {
        let t = typeof e;
        if (t == `number` || t == `boolean` || e == null) return `${e}`;
        if (t == `string`) return `"${e}"`;
        if (t == `symbol`) {
            let t = e.description;
            return t == null ? `Symbol` : `Symbol(${t})`;
        }
        if (t == `function`) {
            let t = e.name;
            return typeof t == `string` && t.length > 0 ? `Function(${t})` : `Function`;
        }
        if (Array.isArray(e)) {
            let t = e.length, n = `[`;
            t > 0 && (n += Gu(e[0]));
            for(let r = 1; r < t; r++)n += `, ` + Gu(e[r]);
            return n += `]`, n;
        }
        let n = /\[object ([^\]]+)\]/.exec(toString.call(e)), r;
        if (n && n.length > 1) r = n[1];
        else return toString.call(e);
        if (r == `Object`) try {
            return `Object(` + JSON.stringify(e) + `)`;
        } catch  {
            return `Object`;
        }
        return e instanceof Error ? `${e.name}: ${e.message}\n${e.stack}` : r;
    }
    function P(e, t) {
        return e >>>= 0, Xu().subarray(e / 4, e / 4 + t);
    }
    function Ku(e, t) {
        return e >>>= 0, Zu().subarray(e / 2, e / 2 + t);
    }
    function F(e, t) {
        return e >>>= 0, Qu().subarray(e / 4, e / 4 + t);
    }
    function qu(e, t) {
        return e >>>= 0, $u().subarray(e / 1, e / 1 + t);
    }
    function Ju(e, t) {
        return e >>>= 0, td().subarray(e / 2, e / 2 + t);
    }
    function I(e, t) {
        return e >>>= 0, nd().subarray(e / 4, e / 4 + t);
    }
    function Yu(e, t) {
        return e >>>= 0, K().subarray(e / 1, e / 1 + t);
    }
    var L = null;
    function R() {
        return (L === null || L.buffer.detached === !0 || L.buffer.detached === void 0 && L.buffer !== $.memory.buffer) && (L = new DataView($.memory.buffer)), L;
    }
    var z = null;
    function Xu() {
        return (z === null || z.byteLength === 0) && (z = new Float32Array($.memory.buffer)), z;
    }
    var B = null;
    function Zu() {
        return (B === null || B.byteLength === 0) && (B = new Int16Array($.memory.buffer)), B;
    }
    var V = null;
    function Qu() {
        return (V === null || V.byteLength === 0) && (V = new Int32Array($.memory.buffer)), V;
    }
    var H = null;
    function $u() {
        return (H === null || H.byteLength === 0) && (H = new Int8Array($.memory.buffer)), H;
    }
    function U(e, t) {
        return sd(e >>> 0, t);
    }
    var ed = null;
    function td() {
        return (ed === null || ed.byteLength === 0) && (ed = new Uint16Array($.memory.buffer)), ed;
    }
    var W = null;
    function nd() {
        return (W === null || W.byteLength === 0) && (W = new Uint32Array($.memory.buffer)), W;
    }
    var G = null;
    function K() {
        return (G === null || G.byteLength === 0) && (G = new Uint8Array($.memory.buffer)), G;
    }
    function q(e, t) {
        try {
            return e.apply(this, t);
        } catch (e) {
            let t = N(e);
            $.__wbindgen_exn_store(t);
        }
    }
    function J(e) {
        return e == null;
    }
    function rd(e, t, n) {
        let r = {
            a: e,
            b: t,
            cnt: 1
        }, i = (...e)=>{
            r.cnt++;
            let t = r.a;
            r.a = 0;
            try {
                return n(t, r.b, ...e);
            } finally{
                r.a = t, i._wbg_cb_unref();
            }
        };
        return i._wbg_cb_unref = ()=>{
            --r.cnt === 0 && ($.__wbindgen_destroy_closure(r.a, r.b), r.a = 0, Wu.unregister(r));
        }, Wu.register(i, r, r), i;
    }
    function Y(e, t, n) {
        if (n === void 0) {
            let n = Z.encode(e), r = t(n.length, 1) >>> 0;
            return K().subarray(r, r + n.length).set(n), Q = n.length, r;
        }
        let r = e.length, i = t(r, 1) >>> 0, a = K(), o = 0;
        for(; o < r; o++){
            let t = e.charCodeAt(o);
            if (t > 127) break;
            a[i + o] = t;
        }
        if (o !== r) {
            o !== 0 && (e = e.slice(o)), i = n(i, r, r = o + e.length * 3, 1) >>> 0;
            let t = K().subarray(i + o, i + r), a = Z.encodeInto(e, t);
            o += a.written, i = n(i, r, o, 1) >>> 0;
        }
        return Q = o, i;
    }
    function id(e) {
        let t = $.__wbindgen_externrefs.get(e);
        return $.__externref_table_dealloc(e), t;
    }
    var X = new TextDecoder(`utf-8`, {
        ignoreBOM: !0,
        fatal: !0
    });
    X.decode();
    var ad = 2146435072, od = 0;
    function sd(e, t) {
        return od += t, od >= ad && (X = new TextDecoder(`utf-8`, {
            ignoreBOM: !0,
            fatal: !0
        }), X.decode(), od = t), X.decode(K().subarray(e, e + t));
    }
    var Z = new TextEncoder;
    `encodeInto` in Z || (Z.encodeInto = function(e, t) {
        let n = Z.encode(e);
        return t.set(n), {
            read: e.length,
            written: n.length
        };
    });
    var Q = 0, $;
    function cd(e) {
        $ = e;
    }
    var ld = t({
        __abort_handler: ()=>gd,
        __externref_table_alloc: ()=>Td,
        __externref_table_dealloc: ()=>kd,
        __instance_terminated: ()=>_d,
        __wbg_dashboard_free: ()=>dd,
        __wbindgen_destroy_closure: ()=>Od,
        __wbindgen_exn_store: ()=>wd,
        __wbindgen_externrefs: ()=>Ed,
        __wbindgen_free: ()=>Dd,
        __wbindgen_malloc: ()=>Sd,
        __wbindgen_realloc: ()=>Cd,
        __wbindgen_start: ()=>Ad,
        dashboard_render: ()=>fd,
        dashboard_resize: ()=>pd,
        initialize_dashboard: ()=>md,
        memory: ()=>ud,
        start: ()=>hd,
        wasm_bindgen__convert__closures_____invoke__h1d8a7b17cad35d54: ()=>yd,
        wasm_bindgen__convert__closures_____invoke__h5ed47cf580eaf96c: ()=>vd,
        wasm_bindgen__convert__closures_____invoke__h602f04d6d0a72043: ()=>bd,
        wasm_bindgen__convert__closures_____invoke__h951a2592665c9e68: ()=>xd
    });
    URL = globalThis.URL;
    var { memory: ud, __wbg_dashboard_free: dd, dashboard_render: fd, dashboard_resize: pd, initialize_dashboard: md, start: hd, __abort_handler: gd, __instance_terminated: _d, wasm_bindgen__convert__closures_____invoke__h5ed47cf580eaf96c: vd, wasm_bindgen__convert__closures_____invoke__h1d8a7b17cad35d54: yd, wasm_bindgen__convert__closures_____invoke__h602f04d6d0a72043: bd, wasm_bindgen__convert__closures_____invoke__h951a2592665c9e68: xd, __wbindgen_malloc: Sd, __wbindgen_realloc: Cd, __wbindgen_exn_store: wd, __externref_table_alloc: Td, __wbindgen_externrefs: Ed, __wbindgen_free: Dd, __wbindgen_destroy_closure: Od, __externref_table_dealloc: kd, __wbindgen_start: Ad } = await _t({
        "./tempest_wasm_bg.js": {
            __wbg_dashboard_new: mr,
            __wbg_log_c224067fcc1a24f4: la,
            __wbg_call_5575218572ead796: hn,
            __wbg_new_typed_00a409eb4ec4f2d9: ga,
            __wbg_push_f724b5db8acf89d2: Ea,
            __wbg_set_a_66601ffa2f4cbde8: Qa,
            __wbg_set_b_103abfb3e69345a3: so,
            __wbg_set_g_a39877021b450e75: ns,
            __wbg_set_r_40fe44b2d9a401f4: Fs,
            __wbg_set_load_op_e8ff3e1c81f7398d: ys,
            __wbg_set_store_op_a95e8da4555c6010: nc,
            __wbg_set_view_506e5beadab34e99: gc,
            __wbg_set_clear_value_c1a82bbe9a80b6ab: yo,
            __wbg_set_resolve_target_6e7eda03a6886624: Rs,
            __wbg_set_color_attachments_6705c6b1e98a3040: So,
            __wbg_set_label_325c5e4b70c1568f: ls,
            __wbg_set_view_32a8132aec6de194: hc,
            __wbg_set_depth_clear_value_25268aa6b7cae2e0: ko,
            __wbg_set_depth_load_op_ed90e4eaf314a16c: Mo,
            __wbg_set_depth_store_op_8e9b1d0e47077643: Io,
            __wbg_set_depth_read_only_90cca09674f446be: No,
            __wbg_set_stencil_clear_value_1f380af0bd0d9255: qs,
            __wbg_set_stencil_load_op_5cde31e71a964b58: Ys,
            __wbg_set_stencil_store_op_262e1df7b92404d3: Qs,
            __wbg_set_stencil_read_only_ac984029b821315e: Zs,
            __wbg_set_depth_stencil_attachment_be8301fa499cd3db: Po,
            __wbg_set_query_set_62d86bdf10d64d37: Ps,
            __wbg_set_beginning_of_pass_write_index_abea1e4e6c6095e1: uo,
            __wbg_set_end_of_pass_write_index_1cd39b9bafe090cc: Vo,
            __wbg_set_timestamp_writes_3854a564715b0ac7: oc,
            __wbg_beginRenderPass_865cbdfaecf89f93: It,
            __wbg_label_9a8583e3a20fafc7: aa,
            __wbg_set_label_8df6673e1e141fcc: gs,
            __wbg_finish_6c7bba424ffe1bbc: si,
            __wbg_finish_c40b67ff2af88e0c: ci,
            __wbg_then_05edfc8a4fea5106: ul,
            __wbg_gpu_cbd27ad0589bc0b3: Yi,
            __wbg_end_d49513b309f4ca43: ii,
            __wbg_onSubmittedWorkDone_5f36409816d68e04: ba,
            __wbg_writeBuffer_24a10bfd5a8a57f7: au,
            __wbg_submit_b3bbead76cbf7627: zc,
            __wbg_mapAsync_e3cfbd141919d03c: ua,
            __wbg_getMappedRange_59829576da3edd39: Oi,
            __wbg_unmap_817a2e3248a553fb: Jl,
            __wbg_destroy_a1ad55d8110037a7: Rr,
            __wbg_createBuffer_3fa0256cba655273: Gn,
            __wbg_createBindGroup_4cb86ff853df5c69: Un,
            __wbg_createShaderModule_f0aa469466c7bdaa: ir,
            __wbg_createPipelineLayout_270b4fd0b4230373: Xn,
            __wbg_createRenderPipeline_4c120add6a62a442: er,
            __wbg_createBindGroupLayout_59891d473ac8665d: Hn,
            __wbg_createCommandEncoder_98e3b731629054b4: qn,
            __wbg_queue_7bbf92178b06da19: Ma,
            __wbg_requestDevice_921f0a221b4492fa: Ha,
            __wbg_instanceof_GpuAdapter_1297a3a5ce0db3ff: Qi,
            __wbg_createView_d04a0f9bdd723238: dr,
            __wbg_set_alpha_bb6680aaf01cdc62: eo,
            __wbg_set_color_495aa415ae5a39c9: xo,
            __wbg_set_module_0933874708065f3b: ws,
            __wbg_set_entry_point_0116a9f5d58cf0aa: Wo,
            __wbg_set_buffers_93f3f75d7338864f: vo,
            __wbg_set_buffer_598ab98a251b8f91: go,
            __wbg_set_offset_e316586bb85f0bd6: Os,
            __wbg_set_size_f1207de283144c72: Ws,
            __wbg_getCurrentTexture_274b67f871b2dea5: wi,
            __wbg_configure_c0a3d80e97c0e7b1: Ln,
            __wbg_instanceof_GpuCanvasContext_13613277d7bf3768: $i,
            __wbg_set_module_a7a131494850e5f7: Ts,
            __wbg_set_entry_point_f04e91eced449196: Go,
            __wbg_set_targets_6664b7e6ec5da9d3: ic,
            __wbg_set_binding_d683cd9c1d4bcfed: po,
            __wbg_set_resource_fe1f979fce4afee2: zs,
            __wbg_set_dst_factor_e44fc612d5e5bff4: Bo,
            __wbg_set_src_factor_c3668d4122497276: Gs,
            __wbg_set_operation_a91e5763a8313c6b: As,
            __wbg_set_front_face_9c9f0518a3109d98: ts,
            __wbg_set_topology_914716698f5868bb: sc,
            __wbg_set_cull_mode_8e533f32672a379b: To,
            __wbg_set_unclipped_depth_e23e3091db2ac351: uc,
            __wbg_set_strip_index_format_62c417aa65a4d277: rc,
            __wbg_set_format_8b8359f261ea64b9: Zo,
            __wbg_set_offset_eabaf12fe1c98ce7: ks,
            __wbg_set_shader_location_03356bf6a6da4332: Hs,
            __wbg_set_mapped_at_creation_7f0aad21612f3e22: bs,
            __wbg_set_size_0c20f73abce8f1ce: Us,
            __wbg_set_usage_41b7d18f3f220e6c: dc,
            __wbg_set_label_37d0faa0c9b7dee4: us,
            __wbg_set_format_119bda0a3d0b3f47: Jo,
            __wbg_set_write_mask_949f521dcf3da2b5: Cc,
            __wbg_set_blend_9eab91d6edf500f9: ho,
            __wbg_set_required_limits_e0de55a49a48e3dc: Ls,
            __wbg_set_required_features_3d00070d09235d7d: Is,
            __wbg_set_label_782e33de78d86641: ms,
            __wbg_set_alpha_to_coverage_enabled_cac9212446be9cab: no,
            __wbg_set_mask_a18cbdfc03a4cbd9: xs,
            __wbg_set_count_34ecf81b3ad7e448: wo,
            __wbg_set_compare_a9a06469832600ec: Co,
            __wbg_set_fail_op_e7eb17ed0228b457: qo,
            __wbg_set_pass_op_eef0c5885ae707c3: js,
            __wbg_set_depth_fail_op_8484012cd5e4987c: jo,
            __wbg_set_depth_bias_07f95aa380a3e46e: Eo,
            __wbg_set_format_a5d373801c562623: Qo,
            __wbg_set_stencil_back_8d01a6c0477059b0: Ks,
            __wbg_set_depth_compare_c017fcac5327dfbb: Ao,
            __wbg_set_stencil_front_f881c15b2d170653: Js,
            __wbg_set_depth_bias_clamp_968b03f74984c77b: Do,
            __wbg_set_stencil_read_mask_d79993adcfc418ab: Xs,
            __wbg_set_stencil_write_mask_94ec6249877e083e: $s,
            __wbg_set_depth_write_enabled_adc2094871d66639: Lo,
            __wbg_set_depth_bias_slope_scale_478b204b4910400f: Oo,
            __wbg_setPipeline_b0ecc74bdf8be629: Xa,
            __wbg_setBindGroup_b546d112a2d27da3: Ya,
            __wbg_draw_92eb37d6b3b2aab4: Qr,
            __wbg_setBindGroup_851043cf286f55f2: Ja,
            __wbg_set_attributes_7ee8e82215809bfa: oo,
            __wbg_set_array_stride_34f4a147a16bff79: io,
            __wbg_set_step_mode_241a8d5515fa964b: ec,
            __wbg_set_layout_d701bf37a1e489c6: vs,
            __wbg_set_entries_f9b7f3d4e9faccf4: Uo,
            __wbg_set_label_5c952448f9d59f36: ps,
            __wbg_set_min_binding_size_d70e460d165d9144: Ss,
            __wbg_set_has_dynamic_offset_69725fed837748fe: rs,
            __wbg_set_type_17a1387b620bc902: cc,
            __wbg_set_alpha_mode_84140629c3b15c51: to,
            __wbg_set_device_47147a331245777f: Ro,
            __wbg_set_format_75eb905a003c2f61: Xo,
            __wbg_set_view_formats_4d0b943f593dd219: yc,
            __wbg_set_usage_6ae4d85589906117: fc,
            __wbg_set_buffer_73d9f6fea9c41867: _o,
            __wbg_set_visibility_bbbf3d2b70571950: bc,
            __wbg_set_binding_e9ba14423117de0a: mo,
            __wbg_set_sampler_12544c21977075c1: Vs,
            __wbg_set_texture_738e6f6215515de3: ac,
            __wbg_set_storage_texture_36be4834c501acab: tc,
            __wbg_set_external_texture_cf122b1392d58f37: Ko,
            __wbg_set_type_d4edb621ec2051e0: lc,
            __wbg_set_sample_type_3cecbd4699e2e5fb: Bs,
            __wbg_set_multisampled_4ce4c32144215354: Ds,
            __wbg_set_view_dimension_9ae69db849267b1a: vc,
            __wbg_set_power_preference_7d669fb9b41f7bf2: Ms,
            __wbg_set_aspect_e09cb246c2df6f46: ao,
            __wbg_set_format_b08d87d5f33bcd89: $o,
            __wbg_set_dimension_b4da3979dc699ef8: zo,
            __wbg_set_base_mip_level_43e77e5d237ede24: lo,
            __wbg_set_mip_level_count_04af0d33c4905fac: Cs,
            __wbg_set_base_array_layer_ff3450be9aa7d232: co,
            __wbg_set_array_layer_count_01e36293bee85e02: ro,
            __wbg_set_label_26577513096f145b: os,
            __wbg_set_usage_f084cd416060ceee: pc,
            __wbg_set_code_6a0d763da082dcfb: bo,
            __wbg_set_label_837a3b8ff99c2db3: hs,
            __wbg_set_label_3e306b2e8f9db666: ds,
            __wbg_set_bind_group_layouts_078241cf2822c39e: fo,
            __wbg_set_label_58fbc9fcc6363f16: fs,
            __wbg_set_layout_a6ee8e74696bc0c8: _s,
            __wbg_set_vertex_29812f650590fa45: mc,
            __wbg_set_multisample_e857cbfca335c7f1: Es,
            __wbg_set_fragment_41044c9110c69c90: es,
            __wbg_set_depth_stencil_d536398c1b29bb38: Fo,
            __wbg_set_primitive_3462e090c7a78969: Ns,
            __wbg_set_label_2816ddca7866dcfa: ss,
            __wbg_set_entries_070b048e4bea0c29: Ho,
            __wbg_set_label_2a41a6f671383447: cs,
            __wbg_set_access_08d6bdbda9aaa266: $a,
            __wbg_set_format_27c63de9b0ec1cb3: Yo,
            __wbg_set_view_dimension_4a840560a13b4860: _c,
            __wbg_getPreferredCanvasFormat_6f629398d892f0c9: ji,
            __wbg_requestAdapter_0049683abd339828: Ba,
            __wbg_Window_65ef42d29dc8174d: bt,
            __wbg_WorkerGlobalScope_d272430d4a323303: xt,
            __wbg_length_c6054974c0a6cdb9: oa,
            __wbg_get_unchecked_54a4374c38e08460: Ji,
            __wbg_includes_83dff8d05da243c5: Zi,
            __wbg_instanceof_Window_0d356b88a2f77c42: na,
            __wbg_performance_4c23a97261596fec: xa,
            __wbg_requestAnimationFrame_b92ccfbc8ca777a8: Va,
            __wbg_document_2634180a4c694068: Ur,
            __wbg_navigator_935098efd1dc7fe5: fa,
            __wbg_clientWidth_128226e900ef22eb: Dn,
            __wbg_clientHeight_af66ce6b5259204b: Tn,
            __wbg_querySelector_1f3658f4b48e268b: ka,
            __wbg_getElementById_c7aba6b93b34bf01: Ti,
            __wbg_querySelectorAll_ffda3c891a9eb29a: Oa,
            __wbg_get_37b48b8fa52d1f2c: qi,
            __wbg_now_0f628e0e435c541b: va,
            __wbg_framebufferTextureMultiviewOVR_bab62b45b7debf2c: gi,
            __wbg_set_height_ef298446b359b0c5: as,
            __wbg_getContext_70c2d1bed75d4122: Si,
            __wbg_getContext_422b32d0ee4b8076: bi,
            __wbg_set_width_f9e631f4ee129e5c: Sc,
            __wbg_drawBuffersWEBGL_3bfb30349766d902: Jr,
            __wbg_set_height_ad5056ea051acd78: is,
            __wbg_getContext_486aab500e1c34c9: xi,
            __wbg_getContext_9fd4db9b1cf155db: Ci,
            __wbg_width_c8740d5bdf596189: iu,
            __wbg_height_a04613570d793df2: Xi,
            __wbg_set_width_031bdecd763c5855: xc,
            __wbg_instanceof_HtmlCanvasElement_8ce29a370a2b10a4: ea,
            __wbg_navigator_017bc45e84c473cc: da,
            __wbg_drawArraysInstancedANGLE_83c84d616f54261b: Wr,
            __wbg_vertexAttribDivisorANGLE_ffd803d04b545670: Zl,
            __wbg_drawElementsInstancedANGLE_a73eba5955ee33fa: Xr,
            __wbg_bindVertexArrayOES_1cb63a86715ea7d5: Yt,
            __wbg_createVertexArrayOES_4bbd1b38563aab57: lr,
            __wbg_deleteVertexArrayOES_287cf2a2e8a27b13: Ar,
            __wbg_queryCounterEXT_ebb00bcc96221671: Da,
            __wbg_blendFunc_713c504adab14f98: on,
            __wbg_colorMask_b0ab9d429a1efa0a: On,
            __wbg_depthFunc_eaca1bc79f7bf216: Nr,
            __wbg_depthMask_3a9074b08d1f68e5: Pr,
            __wbg_frontFace_c9bb1fa659ffd276: vi,
            __wbg_bindBuffer_e95efaf0d4851845: Vt,
            __wbg_blendColor_a0ba1cdcecc3a34c: Qt,
            __wbg_clearDepth_ac6b54f112feeaf7: bn,
            __wbg_depthRange_c24a808b3496e0a9: Lr,
            __wbg_drawArrays_10e1254aa4524ae9: Kr,
            __wbg_useProgram_0e1cd86765304939: Yl,
            __wbg_bindTexture_ffc56f1e5c5526c6: Jt,
            __wbg_linkProgram_6a2eee02a03b9b00: sa,
            __wbg_pixelStorei_55ad4c67b699537c: Sa,
            __wbg_stencilMask_f9fe198f7fd6fc9c: Ic,
            __wbg_attachShader_683d1070365e7066: Pt,
            __wbg_clearStencil_56d6a6308294a749: xn,
            __wbg_createBuffer_0e42c2e1f7bbaeeb: Wn,
            __wbg_createShader_27d9388313f3b14e: ar,
            __wbg_deleteBuffer_d758283bea6e0ccf: gr,
            __wbg_deleteShader_79dbaaed69b7ca3b: Er,
            __wbg_getParameter_91a344e1a84a4669: Ai,
            __wbg_shaderSource_628c37a476ae65f9: wc,
            __wbg_activeTexture_1d5359b20df41710: jt,
            __wbg_blendEquation_8bfa69f639ae92da: nn,
            __wbg_compileShader_fcf3f3d2891f73f9: jn,
            __wbg_createProgram_f11c63f59f41b82a: Qn,
            __wbg_createTexture_8455c703424c567b: cr,
            __wbg_deleteProgram_0135c6926e75af75: yr,
            __wbg_deleteTexture_3acb672a45f9998a: Or,
            __wbg_polygonOffset_06dc6468c12a57e1: wa,
            __wbg_texParameteri_fe6210a493d48a16: Kc,
            __wbg_bindFramebuffer_217a1f4d28c6bc77: Ht,
            __wbg_bindRenderbuffer_d608b211c51ed147: Gt,
            __wbg_createFramebuffer_ab73f30b5dc97415: Yn,
            __wbg_deleteFramebuffer_8953b325144192fe: vr,
            __wbg_blendFuncSeparate_3eef699c291dbc87: rn,
            __wbg_createRenderbuffer_556000dbb01f5026: tr,
            __wbg_deleteRenderbuffer_ada86fd85d32984f: Sr,
            __wbg_getShaderInfoLog_f423ce6d280ccca0: Ri,
            __wbg_stencilOpSeparate_6cf50803475d2640: Rc,
            __wbg_bindAttribLocation_d56d3c40331af7ed: Rt,
            __wbg_bufferData_5df9bdb32e189eee: un,
            __wbg_getProgramInfoLog_35410850de9ccefe: Mi,
            __wbg_getShaderParameter_93cc1f20f1dd0b1e: Bi,
            __wbg_getUniformLocation_340155dc706d3fea: Gi,
            __wbg_renderbufferStorage_2918fb696fd45663: Ra,
            __wbg_copyTexSubImage2D_4b1ba73bf053b4e6: Bn,
            __wbg_getProgramParameter_bbc667347ac2e882: Fi,
            __wbg_stencilFuncSeparate_a17b2b1cc34fa948: jc,
            __wbg_stencilMaskSeparate_3bf2cb54cc370b58: Nc,
            __wbg_framebufferTexture2D_3a558e14f56720d2: mi,
            __wbg_blendEquationSeparate_aed0a34303d3e6ae: en,
            __wbg_framebufferRenderbuffer_52ae9fcc29125a07: di,
            __wbg_uniform2fv_39277cf2d3cb83c7: yl,
            __wbg_uniform2iv_c5e863975dd780d8: bl,
            __wbg_uniform3fv_7723e142be50856f: Cl,
            __wbg_uniform3iv_bea3976522e15d48: Tl,
            __wbg_uniform4fv_85ad5d23234895d2: Al,
            __wbg_uniform4iv_a9aaa92f2f458ec2: Nl,
            __wbg_enableVertexAttribArray_7fc50a6fdbc03eb3: ei,
            __wbg_disableVertexAttribArray_25f8b2d699a4387a: zr,
            __wbg_vertexAttribPointer_36c76a0c7e4f0239: eu,
            __wbg_uniformMatrix2fv_cd6e1725152efce9: Ll,
            __wbg_uniformMatrix3fv_da8c388748c5739b: Vl,
            __wbg_uniformMatrix4fv_c8ba105f2ce3edf8: Gl,
            __wbg_bufferData_26132561617ce8fb: ln,
            __wbg_readPixels_05377f8b6fa1d8eb: Pa,
            __wbg_bufferSubData_44db8a3b4a70b57d: mn,
            __wbg_compressedTexSubImage2D_44c06107dab236a8: Mn,
            __wbg_clear_ff8cfdf420f7dde6: wn,
            __wbg_flush_eb3fb8da2ec00d57: ui,
            __wbg_enable_c6e523307311617a: ti,
            __wbg_texSubImage2D_1b2ff28f994c325e: Yc,
            __wbg_disable_4dca6ee0ccc91e4a: Vr,
            __wbg_scissor_f9696c630e464977: qa,
            __wbg_texImage2D_7bc3001cb8602ed2: Hc,
            __wbg_viewport_b5bd46a0d111c83c: ru,
            __wbg_cullFace_774780fb4177aab8: fr,
            __wbg_uniform1f_d34e4b454f7c8e73: ml,
            __wbg_uniform1i_cd9a7f990128ea48: hl,
            __wbg_uniform4f_4410d2faaa7e5dda: Ol,
            __wbg_instanceof_WebGl2RenderingContext_b30fc72a0130431a: ta,
            __wbg_blendFunc_b15af02643e188f1: sn,
            __wbg_colorMask_f111e3e5796458f4: kn,
            __wbg_depthFunc_31b183b5b8ee478e: Mr,
            __wbg_depthMask_cab7f2ae7f0e559c: Fr,
            __wbg_fenceSync_56efc7cc79111e54: oi,
            __wbg_frontFace_9bdcf2a758e989e5: _i,
            __wbg_uniform1ui_36c94692177ebf76: _l,
            __wbg_beginQuery_dad334d972fed3cc: Ft,
            __wbg_bindBuffer_d3111de6861cb875: Bt,
            __wbg_blendColor_15f26633b646e542: Zt,
            __wbg_clearDepth_05c17028494ee4dd: yn,
            __wbg_deleteSync_37ca83c429c43d8a: Dr,
            __wbg_depthRange_797a71ba3b79267a: Ir,
            __wbg_drawArrays_42dbb4b0349c8f34: qr,
            __wbg_readBuffer_361ec5474f3aae49: Na,
            __wbg_useProgram_ab2ee2a13a1fd909: Xl,
            __wbg_bindSampler_3c7002cb6d56ae8f: Kt,
            __wbg_bindTexture_7ab28ff4ff3dc506: qt,
            __wbg_createQuery_f013132b870a71ef: $n,
            __wbg_deleteQuery_ef51ea0a52420103: xr,
            __wbg_drawBuffers_558d96e52e754731: Yr,
            __wbg_linkProgram_e23a348b0f6e0c4f: ca,
            __wbg_pixelStorei_a78a504be58d1d0a: Ca,
            __wbg_stencilMask_485dcb5965c79a71: Fc,
            __wbg_attachShader_3477e67517b09b6b: Nt,
            __wbg_clearStencil_917833d1e2ac56e4: Sn,
            __wbg_createBuffer_9f602b2dbcbf409c: Kn,
            __wbg_createShader_9c5cd42709d915ff: or,
            __wbg_deleteBuffer_d0fb5f1492ee8c6f: hr,
            __wbg_deleteShader_3339454254c7147c: Tr,
            __wbg_getExtension_11824edd67a143d8: Ei,
            __wbg_getParameter_7f7f23cae98f2c81: ki,
            __wbg_shaderSource_66dce75b25a1a407: Tc,
            __wbg_activeTexture_525ee3068cb9e8d5: Mt,
            __wbg_blendEquation_7dadb4db540a42da: tn,
            __wbg_compileShader_f5625b583b2c9fd6: An,
            __wbg_createProgram_4c8164d471c10346: Zn,
            __wbg_createSampler_9fe50152a2524319: rr,
            __wbg_createTexture_3eed23cb87dd35fc: sr,
            __wbg_deleteProgram_eff668280dcb01ca: br,
            __wbg_deleteSampler_5c045e0cc55813d4: wr,
            __wbg_deleteTexture_c1c58550dc55af5c: kr,
            __wbg_polygonOffset_a4f07d97b9b0dced: Ta,
            __wbg_texParameteri_c6efffcecb474d2f: Gc,
            __wbg_texStorage2D_ed5df596c5f1e3af: qc,
            __wbg_texStorage3D_160b0197bc190f04: Jc,
            __wbg_bindFramebuffer_63e837a5dc0accfb: Ut,
            __wbg_blitFramebuffer_ea96ada8bba07582: cn,
            __wbg_bindRenderbuffer_7f84d28a1462a95a: Wt,
            __wbg_bindVertexArray_c391bd47303d75cd: Xt,
            __wbg_createFramebuffer_4a250944a4542bbc: Jn,
            __wbg_deleteFramebuffer_4c0996be4bc30a67: _r,
            __wbg_getSyncParameter_9a2bda340ebe166f: Ui,
            __wbg_samplerParameterf_3157ba41c0f4d97a: Wa,
            __wbg_samplerParameteri_f85a29156e790189: Ga,
            __wbg_blendFuncSeparate_456410f9919bed39: an,
            __wbg_createRenderbuffer_5ada3a0bc7cf3a43: nr,
            __wbg_createVertexArray_8685feb21901c932: ur,
            __wbg_deleteRenderbuffer_c9320d711ddf649b: Cr,
            __wbg_deleteVertexArray_8ee078fdb1fb1ffe: jr,
            __wbg_getQueryParameter_0599e85ddb81220b: Ii,
            __wbg_getShaderInfoLog_495bddda98172699: Li,
            __wbg_stencilOpSeparate_5c4dbe1cf597c5ed: Lc,
            __wbg_bindAttribLocation_79b5d26727094518: Lt,
            __wbg_bufferData_64e9905f2b3d3a6f: dn,
            __wbg_getProgramInfoLog_cd84be80942f345b: Ni,
            __wbg_getShaderParameter_4eb65cfb174ceb22: zi,
            __wbg_getUniformLocation_ab63f569a4e41744: Ki,
            __wbg_readPixels_5bf204799ed2272f: Ia,
            __wbg_renderbufferStorage_3049e13db5c4e60e: za,
            __wbg_copyTexSubImage2D_3c7de20db5e2b39f: zn,
            __wbg_copyTexSubImage3D_8ba04135d122a27a: Vn,
            __wbg_drawArraysInstanced_999df3e7f5c8762b: Gr,
            __wbg_getIndexedParameter_2b18df6fca85f751: Di,
            __wbg_getProgramParameter_039391d5ba319f50: Pi,
            __wbg_stencilFuncSeparate_ec603976be9569a4: Mc,
            __wbg_stencilMaskSeparate_cabcdf843acbf5f1: Pc,
            __wbg_texImage3D_30dbf7234481b8f5: Uc,
            __wbg_uniformBlockBinding_829a71912ad79a04: Fl,
            __wbg_vertexAttribDivisor_f17a8585267be92f: Ql,
            __wbg_framebufferTexture2D_367ab597a005e8d9: pi,
            __wbg_invalidateFramebuffer_08fe15b00b070e47: ra,
            __wbg_blendEquationSeparate_8240ddfa32266109: $t,
            __wbg_getUniformBlockIndex_3bf387d80cee898d: Wi,
            __wbg_framebufferRenderbuffer_f9d75924fbe9024a: fi,
            __wbg_getSupportedExtensions_fbc6e8f81b1f5dbd: Vi,
            __wbg_clientWaitSync_7580165bd2eff461: En,
            __wbg_framebufferTextureLayer_2312acdc74f97676: hi,
            __wbg_texSubImage3D_933e4cd41cc5376d: sl,
            __wbg_uniform2fv_17592f4dad9798fb: vl,
            __wbg_uniform2iv_d5d29ebbc466977d: xl,
            __wbg_uniform3fv_dbe44b778e6b89e8: wl,
            __wbg_uniform3iv_d75d3a5f86d54be4: El,
            __wbg_uniform4fv_9e670a001c77dca0: jl,
            __wbg_uniform4iv_1d57c6b8e5c0e447: Ml,
            __wbg_enableVertexAttribArray_4f0f3da1ae1fd116: $r,
            __wbg_uniform2uiv_418ba3bf6a230dd5: Sl,
            __wbg_uniform3uiv_71c1efc24a662de9: Dl,
            __wbg_uniform4uiv_c5d45f5dbdae727a: Pl,
            __wbg_disableVertexAttribArray_b395358ec5084c39: Br,
            __wbg_clearBufferfv_6b77b9402254a2bf: gn,
            __wbg_clearBufferiv_0f056544010eef3e: _n,
            __wbg_clearBufferuiv_1d2d93401c0904a3: vn,
            __wbg_vertexAttribPointer_4e5d289c5d224210: tu,
            __wbg_drawElementsInstanced_fdc96cf6adbebc12: Zr,
            __wbg_renderbufferStorageMultisample_d64a8abb8689a968: La,
            __wbg_texSubImage3D_bd32c8e29e470904: cl,
            __wbg_uniformMatrix2fv_b666dc80e084ddbc: Il,
            __wbg_uniformMatrix3fv_2ccfe6ff9f4f57ec: Bl,
            __wbg_uniformMatrix4fv_61b1a000cfdc35cc: Wl,
            __wbg_vertexAttribIPointer_23b6d6b8b8b79b4d: $l,
            __wbg_bindBufferRange_16a9d90becc2a7d7: zt,
            __wbg_bufferData_99bbbc63f02251c4: fn,
            __wbg_texSubImage3D_355efde0dc047913: al,
            __wbg_uniformMatrix2x3fv_6a6221d5300ad184: Rl,
            __wbg_uniformMatrix2x4fv_68bc9cd1e2d67339: zl,
            __wbg_uniformMatrix3x2fv_1083b1ecb80866a1: Hl,
            __wbg_uniformMatrix3x4fv_d4cc158d92dbd1ce: Ul,
            __wbg_uniformMatrix4x2fv_cb66ed882d29c550: Kl,
            __wbg_uniformMatrix4x3fv_99e2e5fabf39e8b6: ql,
            __wbg_readPixels_5840000f3e22f3ce: Fa,
            __wbg_texImage3D_8647ef58aef5b912: Wc,
            __wbg_texSubImage3D_0fdbd843482bb916: il,
            __wbg_texSubImage3D_09a97a968b93253d: rl,
            __wbg_texSubImage3D_c3538b28040daac9: ll,
            __wbg_texSubImage3D_5a238709d114b609: ol,
            __wbg_compressedTexSubImage2D_9d66d6214713bbfb: Pn,
            __wbg_compressedTexSubImage3D_73e1f9f3aa71a2da: Fn,
            __wbg_copyBufferSubData_a944f33b601b822d: Rn,
            __wbg_bufferSubData_2270f1b9db71e642: pn,
            __wbg_compressedTexSubImage2D_7f963168c14c0082: Nn,
            __wbg_compressedTexSubImage3D_e47c04fef5551d29: In,
            __wbg_getBufferSubData_92680d3a2f7be029: yi,
            __wbg_texSubImage2D_d14d9c0a1f627c31: tl,
            __wbg_clear_dadcb3e2929388b0: Cn,
            __wbg_flush_7ae42f071230db6b: li,
            __wbg_texImage2D_0e537dc331652de3: Vc,
            __wbg_texSubImage2D_ce2585a1bf3d56d9: el,
            __wbg_texSubImage2D_bbd523c9ebd9fa99: $c,
            __wbg_enable_d1f42f78be33a553: ni,
            __wbg_texSubImage2D_5c630043f1c56716: Zc,
            __wbg_texSubImage2D_3884b5a5c27ca5c4: Xc,
            __wbg_texSubImage2D_89866a04ecd0a76b: Qc,
            __wbg_texSubImage2D_f03448e182b0820d: nl,
            __wbg_disable_cb1b3e6c1cee5202: Hr,
            __wbg_scissor_6c024669fbf4fe72: Ka,
            __wbg_texImage2D_0e0f37b9fb297d01: Bc,
            __wbg_viewport_a0ca330f9b85397e: nu,
            __wbg_cullFace_94f24b4fd5e9038b: pr,
            __wbg_endQuery_161170c5280a8293: ri,
            __wbg_uniform1f_3acd3f3eb50b5e11: pl,
            __wbg_uniform1i_e4f13604354c28ae: gl,
            __wbg_uniform4f_a5008773cfb47d1a: kl,
            __wbg_getSupportedProfiles_e24289cb9a71b3f0: Hi,
            __wbg_new_36e147a8ced3c6e0: ha,
            __wbg_new_2e117a478906f062: ma,
            __wbg_new_with_byte_offset_and_length_f2b65504a914f37a: _a,
            __wbg_of_62183ea089c00bfa: ya,
            __wbg_is_de5b366c746e004c: ia,
            __wbg_static_accessor_GLOBAL_THIS_2fee5048bcca5938: Dc,
            __wbg_static_accessor_SELF_44f6e0cb5e67cdad: kc,
            __wbg_static_accessor_GLOBAL_ce44e66a4935da8c: Oc,
            __wbg_static_accessor_WINDOW_168f178805d978fe: Ac,
            __wbg_then_2a84678a50976959: dl,
            __wbg_set_4564f7dc44fcb0c9: Za,
            __wbg_then_591b6b3a75ee817a: fl,
            __wbg_queueMicrotask_311744e534a929a3: ja,
            __wbg_queueMicrotask_1c9b3800e321a967: Aa,
            __wbg_resolve_d82363d90af6928a: Ua,
            __wbg_new_227d7c05414eb861: pa,
            __wbg_stack_3b0d974bbf31e44f: Ec,
            __wbg_error_a6fa202b58aa1cd3: ai,
            __wbg___wbindgen_string_get_71bb4348194e31f0: Ot,
            __wbg___wbindgen_number_get_1cc01dd708740256: Dt,
            __wbg___wbindgen_throw_ea4887a5f8f9a9db: kt,
            __wbg___wbindgen_is_null_6d937fbfb6478470: Tt,
            __wbg___wbindgen_boolean_get_edaed31a367ce1bd: St,
            __wbg___wbindgen_is_function_acc5528be2b923f2: wt,
            __wbg___wbindgen_is_undefined_721f8decd50c87a3: Et,
            __wbg__wbg_cb_unref_33c39e13d73b25f6: At,
            __wbg___wbindgen_debug_string_8a447059637473e2: Ct,
            __wbindgen_init_externref_table: vu,
            __wbindgen_cast_0000000000000001: ou,
            __wbindgen_cast_0000000000000002: su,
            __wbindgen_cast_0000000000000003: cu,
            __wbindgen_cast_0000000000000004: lu,
            __wbindgen_cast_0000000000000005: uu,
            __wbindgen_cast_0000000000000006: du,
            __wbindgen_cast_0000000000000007: fu,
            __wbindgen_cast_0000000000000008: pu,
            __wbindgen_cast_0000000000000009: mu,
            __wbindgen_cast_000000000000000a: hu,
            __wbindgen_cast_000000000000000b: gu,
            __wbindgen_cast_000000000000000c: _u
        }
    }, gt);
    cd(ld), Ad();
    function jd() {
        let e = (0, w.useRef)(null), { isConnected: t, engineStatus: r, cpu: a, gpu: o, ram: c, tps: l, ctxUsed: u, ctxTotal: d, backgroundIntensity: f, isEditorFocused: p, activeFile: m, setActiveFile: h, isTerminalOpen: g, setTerminalOpen: _, activeTab: v, setActiveTab: ee, chatViewMode: b, setChatViewMode: te, isFileEditable: x, setFileEditable: C } = T();
        ht(), (0, w.useEffect)(()=>{
            let t = setTimeout(()=>{
                e.current && e.current.getSize().asPercentage < 10 && e.current.resize(m ? `20` : `25`);
            }, 100);
            return ()=>clearTimeout(t);
        }, [
            m
        ]), (0, w.useEffect)(()=>{
            let e = (e)=>{
                (e.metaKey || e.ctrlKey) && e.key === `j` && (e.preventDefault(), _(!g));
            };
            return window.addEventListener(`keydown`, e), ()=>window.removeEventListener(`keydown`, e);
        }, [
            g,
            _
        ]);
        let ne = async ()=>{
            try {
                let e = await yt(`vortex-canvas`);
                console.log(`🌪️ WASM Dashboard Online`), window.addEventListener(`resize`, ()=>{
                    let t = document.getElementById(`vortex-canvas`);
                    t && (t.width = window.innerWidth, t.height = window.innerHeight, e.resize(window.innerWidth, window.innerHeight));
                });
            } catch (e) {
                console.error(`❌ Failed to load WASM:`, e);
            }
        };
        return (0, w.useEffect)(()=>{
            ne();
        }, []), (0, E.jsxs)(`div`, {
            className: `h-screen w-screen overflow-hidden flex flex-col relative text-foreground`,
            children: [
                (0, E.jsx)(st, {}),
                (0, E.jsx)(ct, {}),
                (0, E.jsx)(mt, {}),
                (0, E.jsx)(it, {}),
                (0, E.jsx)(`div`, {
                    className: `fixed inset-0 z-[-1] transition-all duration-700 pointer-events-none ${f === `subtle` ? `opacity-20` : f === `medium` ? `opacity-50` : `opacity-100`} ${p ? `blur-[8px] opacity-10 scale-105` : ``}`,
                    children: (0, E.jsx)(`canvas`, {
                        id: `vortex-canvas`,
                        className: `w-full h-full block`
                    })
                }),
                (0, E.jsxs)(`header`, {
                    className: `flex-none h-14 glass-panel border-b border-border/50 flex items-center justify-between px-6 z-10`,
                    children: [
                        (0, E.jsxs)(`div`, {
                            className: `flex items-center gap-3`,
                            children: [
                                (0, E.jsx)(`span`, {
                                    className: `text-xl`,
                                    children: `🌪️`
                                }),
                                (0, E.jsxs)(`h1`, {
                                    className: `text-lg font-semibold tracking-widest`,
                                    children: [
                                        `TEMPEST`,
                                        ` `,
                                        (0, E.jsx)(`span`, {
                                            className: `text-accent drop-shadow-[0_0_8px_rgba(0,242,255,0.4)]`,
                                            children: `AI`
                                        })
                                    ]
                                })
                            ]
                        }),
                        (0, E.jsxs)(`div`, {
                            className: `flex items-center gap-6`,
                            children: [
                                (0, E.jsx)(`button`, {
                                    onClick: ()=>te(b === `classic` ? `timeline` : `classic`),
                                    className: `flex items-center gap-2 px-3 py-1.5 rounded-md border text-xs font-mono transition-all cursor-pointer bg-white/5 border-border text-muted-foreground hover:bg-white/10 hover:text-white`,
                                    children: b === `classic` ? `CLASSIC VIEW` : `TIMELINE VIEW`
                                }),
                                (0, E.jsxs)(`button`, {
                                    onClick: ()=>_(!g),
                                    className: `flex items-center gap-2 px-3 py-1.5 rounded-md border text-xs font-mono transition-all cursor-pointer ${g ? `bg-accent/15 border-accent text-accent shadow-[0_0_10px_rgba(0,242,255,0.15)]` : `bg-white/5 border-border text-muted-foreground hover:bg-white/10 hover:text-white`}`,
                                    children: [
                                        (0, E.jsx)(Ce, {
                                            size: 14
                                        }),
                                        ` TERMINAL`
                                    ]
                                }),
                                (0, E.jsxs)(`div`, {
                                    className: `flex gap-4 font-mono text-xs text-muted-foreground`,
                                    children: [
                                        (0, E.jsxs)(`span`, {
                                            className: `bg-white/5 px-3 py-1 rounded-md border-l-2 border-accent`,
                                            children: [
                                                `CPU: `,
                                                a.toFixed(1),
                                                `%`
                                            ]
                                        }),
                                        (0, E.jsxs)(`span`, {
                                            className: `bg-white/5 px-3 py-1 rounded-md border-l-2 border-accent`,
                                            children: [
                                                `GPU: `,
                                                o.toFixed(1),
                                                `%`
                                            ]
                                        }),
                                        (0, E.jsxs)(`span`, {
                                            className: `bg-white/5 px-3 py-1 rounded-md border-l-2 border-accent`,
                                            children: [
                                                `RAM: `,
                                                c
                                            ]
                                        })
                                    ]
                                })
                            ]
                        })
                    ]
                }),
                (0, E.jsx)(`main`, {
                    className: `flex-1 min-h-0 relative z-10 flex p-4 pb-0`,
                    children: (0, E.jsxs)(i, {
                        orientation: `horizontal`,
                        className: `h-full w-full`,
                        id: `workspace-group`,
                        onLayout: ()=>Xe(),
                        children: [
                            (0, E.jsxs)(n, {
                                id: `sidebar-panel`,
                                panelRef: e,
                                defaultSize: m ? `20` : `25`,
                                minSize: `15`,
                                maxSize: `35`,
                                style: {
                                    minWidth: `240px`,
                                    maxWidth: `400px`
                                },
                                className: `glass-panel border border-border/50 rounded-xl overflow-hidden flex flex-col`,
                                children: [
                                    (0, E.jsx)(`div`, {
                                        className: `flex border-b border-border/50 bg-black/20 p-1 gap-1`,
                                        children: [
                                            {
                                                id: `files`,
                                                label: `Explorer`,
                                                icon: y
                                            },
                                            {
                                                id: `agent`,
                                                label: `Core`,
                                                icon: S
                                            },
                                            {
                                                id: `search`,
                                                label: `Diagnostics`,
                                                icon: Fe
                                            },
                                            {
                                                id: `settings`,
                                                label: `Tuning`,
                                                icon: fe
                                            }
                                        ].map((e)=>{
                                            let t = e.icon, n = v === e.id;
                                            return (0, E.jsxs)(`button`, {
                                                onClick: ()=>{
                                                    j(), ee(e.id);
                                                },
                                                className: `flex-1 py-2 rounded-lg flex flex-col items-center gap-1 text-[10px] uppercase font-bold tracking-wider transition-all cursor-pointer ${n ? `bg-white/10 text-white border border-white/5 shadow-sm` : `text-muted-foreground hover:text-white hover:bg-white/5 border border-transparent`}`,
                                                children: [
                                                    (0, E.jsx)(t, {
                                                        size: 14,
                                                        className: n ? `text-accent` : ``
                                                    }),
                                                    (0, E.jsx)(`span`, {
                                                        children: e.label
                                                    })
                                                ]
                                            }, e.id);
                                        })
                                    }),
                                    (0, E.jsxs)(`div`, {
                                        className: `flex-1 p-4 overflow-y-auto min-h-0`,
                                        children: [
                                            v === `files` && (0, E.jsx)(Ze, {}),
                                            v === `agent` && (0, E.jsx)(nt, {}),
                                            v === `search` && (0, E.jsx)(at, {}),
                                            v === `settings` && (0, E.jsx)(ot, {})
                                        ]
                                    })
                                ]
                            }),
                            (0, E.jsx)(s, {
                                className: `w-1.5 hover:bg-accent/30 transition-all cursor-col-resize relative flex items-center justify-center bg-white/5 border-l border-r border-white/5 duration-150`,
                                children: (0, E.jsx)(`div`, {
                                    className: `w-0.5 h-8 bg-muted-foreground/20 rounded-full`
                                })
                            }),
                            (0, E.jsx)(n, {
                                id: `chat-panel`,
                                defaultSize: m ? `40` : `75`,
                                minSize: `30`,
                                style: {
                                    minWidth: `320px`
                                },
                                className: `glass-panel border border-border/50 rounded-xl overflow-hidden flex flex-col`,
                                children: b === `classic` ? (0, E.jsx)(Qe, {}) : (0, E.jsx)(ft, {})
                            }),
                            m && (0, E.jsxs)(E.Fragment, {
                                children: [
                                    (0, E.jsx)(s, {
                                        className: `w-1.5 hover:bg-accent/30 transition-all cursor-col-resize relative flex items-center justify-center bg-white/5 border-l border-r border-white/5 duration-150`,
                                        children: (0, E.jsx)(`div`, {
                                            className: `w-0.5 h-8 bg-muted-foreground/20 rounded-full`
                                        })
                                    }),
                                    (0, E.jsxs)(n, {
                                        id: `editor-panel`,
                                        defaultSize: `40`,
                                        minSize: `20`,
                                        style: {
                                            minWidth: `300px`
                                        },
                                        className: `glass-panel border border-border/50 rounded-xl overflow-hidden flex flex-col`,
                                        children: [
                                            (0, E.jsxs)(`div`, {
                                                className: `p-3 border-b border-border/50 flex justify-between items-center bg-black/20`,
                                                children: [
                                                    (0, E.jsx)(`span`, {
                                                        className: `font-mono text-sm truncate pr-4`,
                                                        children: m.name
                                                    }),
                                                    (0, E.jsxs)(`div`, {
                                                        className: `flex items-center gap-2`,
                                                        children: [
                                                            (0, E.jsx)(`button`, {
                                                                onClick: ()=>{
                                                                    x && window.sendNexus && m && window.sendNexus(`WriteFile`, {
                                                                        path: m.name,
                                                                        content: m.content
                                                                    }), C(!x);
                                                                },
                                                                className: `text-xs font-semibold px-3 py-1 rounded transition-colors cursor-pointer ${x ? `bg-green-500/20 text-green-400 hover:bg-green-500/30` : `bg-white/10 hover:bg-white/20`}`,
                                                                children: x ? `SAVE` : `EDIT`
                                                            }),
                                                            (0, E.jsx)(`button`, {
                                                                onClick: ()=>h(null),
                                                                className: `text-muted-foreground hover:text-white px-2 py-1 hover:bg-white/5 rounded transition-all cursor-pointer text-sm font-bold`,
                                                                title: `Close File`,
                                                                children: `✕`
                                                            })
                                                        ]
                                                    })
                                                ]
                                            }),
                                            (0, E.jsx)(rt, {})
                                        ]
                                    })
                                ]
                            })
                        ]
                    }, m ? `workspace-3` : `workspace-2`)
                }),
                (0, E.jsxs)(`div`, {
                    className: `absolute bottom-4 left-1/2 -translate-x-1/2 w-[80%] h-64 glass-panel border border-border/50 rounded-t-xl z-30 transition-all duration-300 ${g ? `translate-y-0 opacity-100` : `translate-y-full opacity-0 pointer-events-none`}`,
                    children: [
                        (0, E.jsxs)(`div`, {
                            className: `flex items-center justify-between p-2 border-b border-border/50 bg-black/40`,
                            children: [
                                (0, E.jsx)(`span`, {
                                    className: `font-mono text-xs pl-2`,
                                    children: `🖥️ TERMINAL`
                                }),
                                (0, E.jsx)(`button`, {
                                    onClick: ()=>_(!1),
                                    className: `text-muted-foreground hover:text-white px-2`,
                                    children: `✕`
                                })
                            ]
                        }),
                        (0, E.jsx)(`div`, {
                            className: `h-[calc(100%-40px)] w-full`,
                            children: g && (0, E.jsx)(Ve, {})
                        })
                    ]
                }),
                (0, E.jsxs)(`footer`, {
                    className: `flex-none h-10 glass-panel border-t border-border/50 flex items-center justify-between px-6 z-10 text-xs font-mono text-muted-foreground mt-4`,
                    children: [
                        (0, E.jsxs)(`div`, {
                            className: `flex items-center gap-3`,
                            children: [
                                (0, E.jsx)(`span`, {
                                    className: `w-2 h-2 rounded-full ${t ? `bg-green-500 shadow-[0_0_8px_#00ff88]` : `bg-red-500 shadow-[0_0_8px_#ff0000]`}`
                                }),
                                (0, E.jsxs)(`span`, {
                                    children: [
                                        `Engine: `,
                                        r
                                    ]
                                })
                            ]
                        }),
                        (0, E.jsxs)(`div`, {
                            className: `flex items-center gap-4`,
                            children: [
                                (0, E.jsxs)(`span`, {
                                    children: [
                                        `TPS: `,
                                        l
                                    ]
                                }),
                                (0, E.jsx)(`span`, {
                                    children: `│`
                                }),
                                (0, E.jsxs)(`span`, {
                                    children: [
                                        `CTX: `,
                                        u >= 1024 ? `${(u / 1024).toFixed(1)}k` : u,
                                        `/`,
                                        d >= 1024 ? `${(d / 1024).toFixed(0)}k` : d
                                    ]
                                })
                            ]
                        })
                    ]
                })
            ]
        });
    }
    Be.createRoot(document.getElementById(`root`)).render((0, E.jsx)(w.StrictMode, {
        children: (0, E.jsx)(jd, {})
    }));
})();
