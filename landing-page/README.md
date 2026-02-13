# Polymarket AI Agent - Landing Page

Modern landing page for the Polymarket AI Agent project.

## 🚀 Live Demo

Visit: [https://polymarket-agent-three.vercel.app/](https://polymarket-agent-three.vercel.app/)

## 📦 Features

- **Fully Responsive** - Mobile, tablet, and desktop optimized
- **Fast Loading** - Pure HTML/CSS/JS, no frameworks
- **Modern Design** - Gradient backgrounds, animations, and smooth scrolling
- **SEO Optimized** - Meta tags, Open Graph, and semantic HTML
- **Zero Dependencies** - No build process required

## 🛠️ Local Development

```bash
# Navigate to landing page directory
cd landing-page

# Option 1: Python HTTP Server
python -m http.server 8000

# Option 2: Node HTTP Server
npx http-server -p 8000

# Open browser
# Visit http://localhost:8000
```

## 📤 Deploy to Vercel

### Method 1: Vercel CLI

```bash
# Install Vercel CLI
npm i -g vercel

# Deploy
cd landing-page
vercel --prod
```

### Method 2: GitHub Integration

1. Push `landing-page/` to your GitHub repository
2. Go to [Vercel Dashboard](https://vercel.com/dashboard)
3. Click "New Project"
4. Import your GitHub repository
5. Set root directory to `landing-page`
6. Click "Deploy"

### Method 3: Drag & Drop

1. Go to [Vercel Dashboard](https://vercel.com/dashboard)
2. Drag and drop the `landing-page` folder
3. Done!

## 📁 File Structure

```
landing-page/
├── index.html          # Main landing page
├── vercel.json         # Vercel configuration
├── package.json        # NPM metadata
└── README.md           # This file
```

## 🎨 Customization

### Colors

Edit the CSS variables in `index.html`:

```css
:root {
    --primary: #6366f1;
    --primary-dark: #4f46e5;
    --secondary: #8b5cf6;
    --success: #10b981;
    --danger: #ef4444;
    /* ... */
}
```

### Content

All content is in `index.html`. Edit the HTML directly to:
- Change text and headings
- Add/remove sections
- Update links and CTAs
- Modify statistics

### Styling

All CSS is embedded in `<style>` tag in `index.html`:
- Modify layout and spacing
- Change animations
- Adjust responsive breakpoints
- Update fonts

## 📊 Sections

1. **Hero** - Eye-catching intro with CTA buttons
2. **Stats** - Key metrics (100+ agents, 700+ markets, etc.)
3. **Features** - 9 feature cards with icons
4. **Architecture** - ASCII diagram of system design
5. **Strategies** - Table of 10 preset strategies
6. **Tech Stack** - Technologies used
7. **Quick Start** - Code block with setup instructions
8. **CTA** - Final call-to-action
9. **Footer** - Links and copyright

## 🔧 Configuration

### vercel.json

```json
{
  "version": 2,
  "builds": [
    {
      "src": "index.html",
      "use": "@vercel/static"
    }
  ],
  "routes": [
    {
      "src": "/(.*)",
      "dest": "/index.html"
    }
  ]
}
```

This ensures all routes serve `index.html` (SPA behavior).

## 📱 Responsive Design

Breakpoints:
- **Desktop**: > 768px
- **Tablet**: 768px - 1024px
- **Mobile**: < 768px

Mobile optimizations:
- Hamburger menu (hidden nav links)
- Single column layouts
- Smaller fonts
- Touch-friendly buttons

## 🚀 Performance

- **No external dependencies** - All CSS/JS inline
- **Optimized images** - SVG icons only
- **Minified code** - Can be minified further if needed
- **Lazy loading** - Intersection Observer for animations

## 📄 License

MIT License - Same as the main project.

---

**Built with ❤️ for Polymarket AI Agent**
