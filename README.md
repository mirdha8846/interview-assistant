# 🎯 Interview Helper Pro - Product Documentation

> **For Website Development / Vibe Coding Tools**
> 
> This document contains all information needed to create a professional website for Interview Helper Pro.

---

## 📦 Product Overview

**Interview Helper Pro** is a Windows desktop application that provides real-time AI-powered assistance during technical interviews. It runs invisibly on the user's screen and provides instant answers to interview questions.

### One-Line Pitch
> *"Ace your technical interviews with AI-powered real-time assistance that's completely invisible to screen sharing."*

---

## ✨ Key Features

### 1. 🎧 Real-Time Audio Transcription
- Captures system audio (interviewer's voice from video call)
- Uses **AssemblyAI** for instant speech-to-text
- Works with Zoom, Google Meet, Microsoft Teams, etc.
- **Does NOT capture user's microphone** - only system audio

### 2. 🤖 AI-Powered Answer Generation
- Uses **Google Gemini 2.0 Flash** for instant answers
- Context-aware responses based on user's resume/profile
- Supports coding questions, behavioral questions, system design
- Generates concise, interview-ready answers

### 3. 👻 Stealth Mode (Invisible to Screen Share)
- Overlay is **completely invisible** to screen recording/sharing
- Uses Windows `WDA_EXCLUDEFROMCAPTURE` technology
- Interviewers cannot see the helper on Zoom/Meet/Teams
- Only visible on user's physical screen

### 4. 🎨 Glassy Blue UI
- Semi-transparent overlay (doesn't block view)
- Clean blue theme with high-contrast text
- Resizable and movable window
- Keyboard-controlled (no mouse needed during interview)

### 5. 🔐 Offline License System
- One-time payment, lifetime license
- Device-locked (tied to hardware ID)
- No subscription, no recurring fees
- Works completely offline after activation

---

## ⌨️ Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Alt+Shift+V` | Start live listening mode |
| `Shift+Q` | Send question to AI (get answer) |
| `Shift+S` | Take screenshot |
| `Shift+F` | Analyze screenshot with AI |
| `Shift+T` | Toggle overlay visibility |
| `Alt+Shift+C` | Stop listening mode |
| `Alt+Shift+K` | Exit application |

---

## 💰 Pricing Model

### Recommended Pricing
| Plan | Price | Features |
|------|-------|----------|
| **Lifetime License** | ₹999 / $12 | Full access, one device, lifetime updates |

### What's Included
- ✅ Unlimited usage
- ✅ All future updates
- ✅ Works on 1 device (hardware-locked)
- ✅ Email support

### User's Requirements (They Need)
1. **Google AI API Key** (Free tier: 15 requests/minute)
   - Get from: https://aistudio.google.com/apikey
2. **AssemblyAI API Key** (Free tier: 100 hours/month)
   - Get from: https://www.assemblyai.com/

---

## 🔄 User Journey / How It Works

### Step 1: Download (Free)
```
User visits website → Downloads interview-helper.exe (free)
```

### Step 2: First Launch
```
App shows setup wizard:
1. Enter License Key (requires payment)
2. Enter Google API Key
3. Enter AssemblyAI API Key
→ App saves credentials securely
```

### Step 3: Payment & License
```
User pays on website → Receives Device ID from app
User submits Device ID → Receives License Key
User enters License Key → App activates!
```

### Step 4: Interview Setup (Every Time)
```
Before each interview, user enters:
- Name
- Target Role (e.g., "Senior Software Engineer")
- Resume/Experience summary
→ AI uses this context for personalized answers
```

### Step 5: During Interview
```
1. User joins Zoom/Meet call
2. Presses Alt+Shift+V to start listening
3. AI transcribes interviewer's questions in real-time
4. User presses Shift+Q when question is complete
5. AI generates answer in 2-3 seconds
6. User reads answer from invisible overlay
```

---

## 🔐 License System Details

### Device ID Format
```
XXXX-XXXX-XXXX-XXXX
Example: A3F2-8B4C-1D9E-7F0A
```
Generated from hardware fingerprint (CPU ID, Disk Serial, BIOS UUID)

### License Key Format
```
SM-XXXX-XXXX-XXXX-XXXX
Example: SM-4A2B-9C8D-3E1F-7055
```
HMAC-based signature tied to Device ID

### License Delivery Options
1. **Manual**: You generate key and send via email/WhatsApp
2. **Semi-Auto**: Website form → You generate → Auto-email
3. **Full-Auto**: Backend generates and delivers instantly

---

## 🛠️ Technical Specifications

### System Requirements
- **OS**: Windows 10/11 (64-bit)
- **RAM**: 4GB minimum
- **Storage**: 50MB
- **Internet**: Required for AI features

### Technologies Used
| Component | Technology |
|-----------|------------|
| App Language | Rust |
| UI Framework | Native Win32 API |
| Audio Capture | WASAPI Loopback |
| Speech-to-Text | AssemblyAI Streaming API |
| AI Model | Google Gemini 2.0 Flash |
| Stealth | WDA_EXCLUDEFROMCAPTURE |

### File Structure
```
interview-helper.exe   (Main application, ~10MB)
%LOCALAPPDATA%/InterviewHelper/
  ├── .license          (Encrypted license data)
  ├── api_keys.enc      (Encrypted API keys)
  └── profile.json      (User profile/resume)
```

---

## 🌐 Website Requirements

### Pages Needed

#### 1. Landing Page (/)
- Hero section with product pitch
- Feature highlights (4-6 cards)
- How it works (3-step process)
- Testimonials (if available)
- CTA: "Download Free" button

#### 2. Pricing Page (/pricing)
- Single pricing card (₹999 lifetime)
- Feature comparison
- FAQ section
- CTA: "Buy Now" button

#### 3. Download Page (/download)
- Download button for .exe
- System requirements
- Quick start guide
- "After download" instructions

#### 4. Activation Page (/activate)
- Payment integration (Razorpay/Stripe)
- After payment: "Enter Device ID" form
- Display/Email license key
- Activation instructions

#### 5. Documentation (/docs)
- Getting started guide
- Keyboard shortcuts
- Troubleshooting
- FAQ

### Design Guidelines
- **Color Scheme**: Dark theme with blue accents (#3399FF)
- **Font**: Inter or Segoe UI
- **Style**: Modern, clean, professional
- **Tone**: Confident but not arrogant

---

## 📝 Website Copy Suggestions

### Hero Section
```
Title: "Ace Every Technical Interview"
Subtitle: "AI-powered real-time assistance that's invisible to screen sharing"
CTA: "Download Free"
```

### Feature Headlines
```
1. "Real-Time Transcription" - Hear what they ask, see it in text
2. "AI-Powered Answers" - Get perfect responses in seconds  
3. "100% Invisible" - Undetectable on Zoom, Meet, Teams
4. "One-Time Payment" - No subscription, lifetime access
```

### How It Works
```
Step 1: "Download & Setup" - Install the app, enter your API keys
Step 2: "Start Your Interview" - Join the call, press Alt+Shift+V
Step 3: "Get Instant Answers" - Press Shift+Q, read the answer
```

### FAQ Suggestions
```
Q: Is it detectable?
A: No. Uses Windows stealth technology to hide from screen capture.

Q: What about my voice?
A: We only capture system audio (interviewer), not your microphone.

Q: Do I need coding skills?
A: No. It's a simple .exe file, just download and run.

Q: What if I change my computer?
A: Contact support for license transfer (free, one-time).

Q: Is it legal?
A: Using assistance tools is a personal decision. We don't encourage misrepresentation.
```

---

## 💳 Payment Integration

### Razorpay (Recommended for India)
```javascript
// After successful payment:
1. Show "Enter Device ID" input
2. User pastes Device ID from app
3. Backend generates license OR
4. Manual: Admin receives notification, sends key
```

### Stripe (International)
```javascript
// Same flow as Razorpay
// Supports cards, Apple Pay, Google Pay
```

### Gumroad (Simplest)
```
- Upload .exe file
- Set price ₹999
- After purchase: Show license generation form
- User gets download + license key
```

---

## 📊 Competitive Positioning

### vs. ChatGPT/Claude
- ❌ They require manual copy-paste
- ✅ We have real-time audio capture

### vs. Other Interview Tools
- ❌ They're visible on screen share
- ✅ We're completely invisible

### vs. Browser Extensions
- ❌ Extensions can be detected
- ✅ Native app is undetectable

---

## 🚀 Launch Checklist

### Before Launch
- [ ] Website live with all pages
- [ ] Payment integration working
- [ ] License generation tested
- [ ] Download link working
- [ ] Support email set up

### For Marketing
- [ ] LinkedIn posts ready
- [ ] Reddit posts (r/cscareerquestions, r/interviews)
- [ ] Twitter/X announcement
- [ ] Demo video (screen recording)

---

## 📞 Support Information

### Contact
- Email: [your-email]
- WhatsApp: [your-number]
- Response time: 24 hours

### Common Issues
1. **"License invalid"** → Device ID changed, need re-license
2. **"No audio"** → Check system audio is playing
3. **"AI not responding"** → Check API key validity

---

## 📁 Files Included

```
production/
├── anti-proctor/
│   ├── src/                    # Rust source code
│   ├── Cargo.toml              # Dependencies
│   └── target/release/
│       └── interview-helper.exe  # Built application
├── README.md                   # This file
└── bussiness.md               # Business notes
```

---

## 🔧 For Developer (License Generator)

### Generate License Key
```bash
# Build the license generator
cargo build --release --bin license_generator

# Generate license for a device
./license_generator.exe <DEVICE_ID>

# Output: SM-XXXX-XXXX-XXXX-XXXX
```

### License Algorithm (For Backend)
```
Input: Device ID (XXXX-XXXX-XXXX-XXXX)
Process: HMAC-like hash with secret key
Output: License Key (SM-XXXX-XXXX-XXXX-XXXX)

Secret: "SM_INTERVIEW_HELPER_2024_PREMIUM_LICENSE_KEY"
Rounds: 1000 hash iterations
Format: SM-{hex[0:4]}-{hex[4:8]}-{hex[8:12]}-{hex[12:16]}
```

---

*Last Updated: February 2026*
*Version: 0.2.0*
