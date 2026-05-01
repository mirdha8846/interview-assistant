import { useState, useEffect, useRef } from "react";
import ReactMarkdown from 'react-markdown';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { vscDarkPlus } from 'react-syntax-highlighter/dist/esm/styles/prism';
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { motion, AnimatePresence } from "framer-motion";

interface UserProfile {
  name: string;
  target_role: string;
  summary: string;
  skills: string[];
  experience_years: number;
}

function App() {
  const [statusMsg, setStatusMsg] = useState("");
  const [aiResponse, setAiResponse] = useState("");
  const [transcription, setTranscription] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [showSetup, setShowSetup] = useState(false);
  
  // Profile state
  const [profile, setProfile] = useState<UserProfile>({
    name: "",
    target_role: "",
    summary: "",
    skills: [],
    experience_years: 0
  });

  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    // Load existing profile on start
    invoke<UserProfile>("load_user_profile").then((p) => {
      setProfile(p);
      if (!p.name) setShowSetup(true); // Show setup if not configured
    });
  }, []);

  useEffect(() => {
    if (!statusMsg) return;
    const t = setTimeout(() => setStatusMsg(""), 4000);
    return () => clearTimeout(t);
  }, [statusMsg]);



  useEffect(() => {
    const subs = [
      listen<string>("status-message-update", (e) => setStatusMsg(e.payload)),
      listen<string>("ai-response-update", (e) => {
        setAiResponse(e.payload);
        setIsLoading(false);
      }),
      listen<string>("overlay-text-update", (e) => {
        const txt = e.payload;
        if (txt.includes("...") || txt.includes("⏳") || txt.includes("⚡") || txt.includes("Requesting")) {
          setIsLoading(true);
          setAiResponse("");
        }
      }),
      listen<string>("live-transcription-update", (e) => setTranscription(e.payload)),
      listen<string>("clear-all-buffers", () => {
        setAiResponse("");
        setTranscription("");
        setIsLoading(false);
        setStatusMsg("");
      }),
      listen<string>("scroll-event", (e) => {
        scrollRef.current?.scrollBy({ top: parseInt(e.payload), behavior: "smooth" });
      }),
    ];
    return () => { subs.forEach((s) => s.then((f) => f())); };
  }, []);

  const saveProfile = async () => {
    await invoke("save_user_profile", { profile });
    setShowSetup(false);
    setStatusMsg("Profile Saved ✅");
  };

  return (
    <div style={{
      width: "100vw",
      height: "100vh",
      background: "transparent",
      fontFamily: "'Inter', system-ui, -apple-system, sans-serif",
      WebkitFontSmoothing: "antialiased",
      display: "flex",
      flexDirection: "column",
      overflow: "hidden",
      boxSizing: "border-box",
      padding: "12px",
    }}>

      {/* ── Premium Glass Card ──────── */}
      <div style={{
        flex: 1,
        display: "flex",
        flexDirection: "column",
        background: "rgba(10, 10, 12, 0.85)",
        backdropFilter: "blur(16px)",
        WebkitBackdropFilter: "blur(16px)",
        border: "1px solid rgba(255, 255, 255, 0.1)",
        borderRadius: "16px",
        boxShadow: "0 10px 40px rgba(0, 0, 0, 0.5)",
        position: "relative",
        overflow: "hidden"
      }}>
        {/* Subtle Noise Texture for Premium Glass Look */}
        <div style={{
          position: "absolute",
          top: 0,
          left: 0,
          right: 0,
          bottom: 0,
          opacity: 0.04,
          pointerEvents: "none",
          zIndex: 0,
          backgroundImage: `url("data:image/svg+xml,%3Csvg viewBox='0 0 200 200' xmlns='http://www.w3.org/2000/svg'%3E%3Cfilter id='noiseFilter'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.65' numOctaves='3' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23noiseFilter)'/%3E%3C/svg%3E")`,
        }} />
        
        {/* Setup Modal Overlay */}
        <AnimatePresence>
          {showSetup && (
            <motion.div 
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              style={{
                position: "absolute",
                inset: 0,
                background: "rgba(0,0,0,0.85)",
                backdropFilter: "blur(20px)",
                zIndex: 100,
                display: "flex",
                flexDirection: "column",
                padding: "24px",
                overflowY: "auto",
              }}
              className="no-scrollbar"
            >
              <h2 style={{ color: "#FFF", fontSize: "18px", marginBottom: "20px", fontWeight: 700 }}>Interview Context</h2>
              
              <div style={{ display: "flex", flexDirection: "column", gap: "16px" }}>
                <div>
                  <label style={{ color: "rgba(255,255,255,0.5)", fontSize: "11px", textTransform: "uppercase", letterSpacing: "0.1em", display: "block", marginBottom: "6px" }}>Your Name</label>
                  <input 
                    type="text" 
                    value={profile.name} 
                    onChange={(e) => setProfile({...profile, name: e.target.value})}
                    style={{ width: "100%", background: "rgba(255,255,255,0.05)", border: "1px solid rgba(255,255,255,0.1)", borderRadius: "8px", padding: "10px", color: "#FFF", fontSize: "14px", outline: "none" }}
                    placeholder="e.g. John Doe"
                  />
                </div>

                <div>
                  <label style={{ color: "rgba(255,255,255,0.5)", fontSize: "11px", textTransform: "uppercase", letterSpacing: "0.1em", display: "block", marginBottom: "6px" }}>Interview For (Role)</label>
                  <input 
                    type="text" 
                    value={profile.target_role} 
                    onChange={(e) => setProfile({...profile, target_role: e.target.value})}
                    style={{ width: "100%", background: "rgba(255,255,255,0.05)", border: "1px solid rgba(255,255,255,0.1)", borderRadius: "8px", padding: "10px", color: "#FFF", fontSize: "14px", outline: "none" }}
                    placeholder="e.g. Senior Frontend Engineer"
                  />
                </div>

                <div>
                  <label style={{ color: "rgba(255,255,255,0.5)", fontSize: "11px", textTransform: "uppercase", letterSpacing: "0.1em", display: "block", marginBottom: "6px" }}>Skills / Tech Stack</label>
                  <input 
                    type="text" 
                    value={profile.skills.join(", ")} 
                    onChange={(e) => setProfile({...profile, skills: e.target.value.split(",").map(s => s.trim())})}
                    style={{ width: "100%", background: "rgba(255,255,255,0.05)", border: "1px solid rgba(255,255,255,0.1)", borderRadius: "8px", padding: "10px", color: "#FFF", fontSize: "14px", outline: "none" }}
                    placeholder="React, TypeScript, Node.js"
                  />
                </div>

                <div>
                  <label style={{ color: "rgba(255,255,255,0.5)", fontSize: "11px", textTransform: "uppercase", letterSpacing: "0.1em", display: "block", marginBottom: "6px" }}>Personal Context / Bio</label>
                  <textarea 
                    value={profile.summary} 
                    onChange={(e) => setProfile({...profile, summary: e.target.value})}
                    style={{ width: "100%", background: "rgba(255,255,255,0.05)", border: "1px solid rgba(255,255,255,0.1)", borderRadius: "8px", padding: "10px", color: "#FFF", fontSize: "14px", outline: "none", minHeight: "100px", resize: "none" }}
                    placeholder="Talk about your experience or how you want the AI to represent you..."
                  />
                </div>

                <button 
                  onClick={saveProfile}
                  style={{ 
                    marginTop: "10px",
                    background: "#00F0FF", 
                    color: "#000", 
                    border: "none", 
                    borderRadius: "8px", 
                    padding: "12px", 
                    fontWeight: 700, 
                    cursor: "pointer",
                    fontSize: "14px"
                  }}
                >
                  START INTERVIEW
                </button>
              </div>
            </motion.div>
          )}
        </AnimatePresence>

        {/* Subtle Shine Effect */}
        <div style={{
            position: "absolute",
            top: 0, left: 0, right: 0, height: "1px",
            background: "linear-gradient(90deg, transparent, rgba(255,255,255,0.4), transparent)",
            zIndex: 10
        }} />

        {/* Header */}
        <div style={{
          padding: "12px 16px",
          borderBottom: "1px solid rgba(255, 255, 255, 0.08)",
          display: "flex",
          alignItems: "center",
          gap: "10px",
          flexShrink: 0,
          background: "rgba(255, 255, 255, 0.02)",
        }}>
          <motion.div
            animate={{ scale: [1, 1.2, 1], opacity: [0.8, 1, 0.8] }}
            transition={{ repeat: Infinity, duration: 2, ease: "easeInOut" }}
            style={{
              width: "8px", height: "8px", borderRadius: "50%",
              background: isLoading ? "#FFB800" : aiResponse ? "#00FFA3" : "#00F0FF",
              boxShadow: `0 0 12px ${isLoading ? "#FFB800" : aiResponse ? "#00FFA3" : "#00F0FF"}`,
            }}
          />
          <span style={{
            fontSize: "12px",
            fontWeight: 600,
            letterSpacing: "0.05em",
            color: "rgba(255, 255, 255, 0.9)",
            textTransform: "uppercase",
            flex: 1,
          }}>
            {isLoading ? "Analyzing Context" : aiResponse ? "Intelligent Insights" : "Stealth Standby"}
          </span>
          <div style={{ display: "flex", gap: "8px", alignItems: "center" }}>
            {statusMsg && (
                <motion.span 
                initial={{ opacity: 0, y: -5 }}
                animate={{ opacity: 1, y: 0 }}
                style={{
                    fontSize: "10px",
                    color: "#00F0FF",
                    fontWeight: 600,
                    background: "rgba(0, 240, 255, 0.1)",
                    padding: "2px 8px",
                    borderRadius: "4px",
                }}>{statusMsg}</motion.span>
            )}
            <button 
                onClick={() => setShowSetup(true)}
                style={{ background: "transparent", border: "none", cursor: "pointer", color: "rgba(0,0,0,0.4)", fontSize: "14px" }}
            >⚙️</button>
          </div>
        </div>

        {/* Live Indicator */}
        {transcription && (
          <div style={{
            padding: "8px 16px",
            background: "rgba(255, 255, 255, 0.03)",
            borderBottom: "1px solid rgba(255, 255, 255, 0.05)",
            flexShrink: 0,
          }}>
            <p style={{
              margin: 0,
              fontSize: "13px",
              color: "rgba(255, 255, 255, 0.7)",
              fontWeight: 500,
              display: "flex",
              alignItems: "center",
              gap: "8px"
            }}>
              <span style={{ color: "#FF4D4D", animation: "pulse 1.5s infinite" }}>●</span>
              <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{transcription}</span>
            </p>
          </div>
        )}

        {/* Content Area */}
        <div
          ref={scrollRef}
          style={{
            flex: 1,
            overflowY: "auto",
            overflowX: "hidden",
            padding: "20px",
            display: "flex",
            flexDirection: "column",
            gap: "16px",
          }}
          className="no-scrollbar"
        >
          {isLoading && !aiResponse && (
            <div style={{ flex: 1, display: "flex", alignItems: "center", justifyContent: "center" }}>
               <div style={{ display: "flex", gap: "8px" }}>
                {[0, 1, 2].map(i => (
                  <motion.div
                    key={i}
                    animate={{ y: [0, -10, 0] }}
                    transition={{ repeat: Infinity, duration: 0.6, delay: i * 0.1 }}
                    style={{ width: "6px", height: "6px", borderRadius: "50%", background: "#FFB800" }}
                  />
                ))}
              </div>
            </div>
          )}

          {aiResponse && (
            <motion.div
              initial={{ opacity: 0, scale: 0.98 }}
              animate={{ opacity: 1, scale: 1 }}
              style={{
                fontSize: "16px",
                lineHeight: "1.6",
                color: "#FFFFFF",
                fontWeight: 500,
                maxWidth: "100%",
                textShadow: "0px 1px 2px rgba(0,0,0,0.8)",
              }}
            >
              <ReactMarkdown
                components={{
                  code({node, inline, className, children, ...props}: any) {
                    const match = /language-(\w+)/.exec(className || '')
                    return !inline && match ? (
                      <SyntaxHighlighter
                        {...props}
                        children={String(children).replace(/\n$/, '')}
                        style={vscDarkPlus}
                        language={match[1]}
                        PreTag="div"
                        customStyle={{
                          background: "rgba(10, 15, 30, 0.95)",
                          border: "1px solid rgba(0, 240, 255, 0.4)",
                          borderRadius: "8px",
                          padding: "16px",
                          marginTop: "12px",
                          marginBottom: "12px",
                          boxShadow: "0 4px 12px rgba(0, 240, 255, 0.15)",
                          fontSize: "14px"
                        }}
                      />
                    ) : (
                      <code {...props} className={className} style={{ background: "rgba(255,255,255,0.15)", padding: "2px 4px", borderRadius: "4px", color: "#00F0FF" }}>
                        {children}
                      </code>
                    )
                  }
                }}
              >
                {aiResponse}
              </ReactMarkdown>
            </motion.div>
          )}

          {!aiResponse && !isLoading && (
            <div style={{
              flex: 1,
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              justifyContent: "center",
              gap: "20px",
              opacity: 0.4,
            }}>
              <div style={{ fontSize: "40px", filter: "drop-shadow(0 0 10px rgba(0,240,255,0.5))" }}>🛡️</div>
              <div style={{ 
                fontSize: "12px", 
                fontWeight: 700, 
                letterSpacing: "0.1em", 
                color: "#FFF", 
                textAlign: "center",
                lineHeight: "1.5"
              }}>
                {profile.name ? `HI ${profile.name.toUpperCase()}` : "READY FOR ACTION"}<br/>
                <span style={{ fontSize: "10px", opacity: 0.5, color: "#FFF" }}>Shift+Q to analyze</span>
              </div>
            </div>
          )}
        </div>

        {/* CSS for custom scrollbar hidden */}
        <style dangerouslySetInnerHTML={{ __html: `
          .no-scrollbar::-webkit-scrollbar { display: none; }
          .no-scrollbar { -ms-overflow-style: none; scrollbar-width: none; }
          @keyframes pulse {
            0% { opacity: 1; }
            50% { opacity: 0.4; }
            100% { opacity: 1; }
          }
        `}} />
      </div>
    </div>
  );
}

export default App;
