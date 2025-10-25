# Building and Publishing Your Blog

This guide covers how to build your blog and deploy it to the web.

## Building Your Site

### The Render Command

To generate your static site:

```bash
bckt render
```

This creates the `html/` directory with your complete website. The `html/` directory contains:

- All your rendered blog posts
- Index pages and archives
- Tag pages
- RSS feed
- Search index
- Static assets from your theme

### Build Options

**Incremental build** (default):
```bash
bckt render
```
Only rebuilds changed files. Fast for ongoing updates.

**Force full rebuild**:
```bash
bckt render --force
```
Regenerates everything from scratch. Use when switching themes or after config changes.

**Build posts only**:
```bash
bckt render --posts
```
Updates post content without rebuilding static assets.

**Build static assets only**:
```bash
bckt render --static
```
Updates CSS, JavaScript, and other assets without rebuilding posts.

**Verbose output**:
```bash
bckt render --verbose
```
Shows detailed information about what's being built.

### Clean and Rebuild

To start fresh:

```bash
bckt clean
bckt render
```

The `clean` command removes the `html/` directory and cache.

## Testing Before Publishing

Always preview your site before deploying:

```bash
bckt dev
```

This builds your site and starts a local server at http://127.0.0.1:4000 with automatic reload on changes.

Check for:
- All posts rendering correctly
- Images loading properly
- Links working
- Navigation functioning
- Search working (if enabled)

## Publishing to Static Hosts

Once you've built your site with `bckt render`, the `html/` directory is ready to deploy to any static hosting service.

### GitHub Pages

**Option 1: Commit the html/ directory**

1. Build your site:
```bash
bckt render
```

2. Commit and push:
```bash
git add html/
git commit -m "Update site"
git push
```

3. In your GitHub repository settings:
   - Go to Settings > Pages
   - Set source to "Deploy from a branch"
   - Select your main branch and `/html` folder
   - Click Save

**Option 2: GitHub Actions (Recommended)**

Create `.github/workflows/deploy.yml`:

```yaml
name: Deploy Blog

on:
  push:
    branches: [main]

jobs:
  build-and-deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Install bckt
        run: |
          wget https://github.com/vrypan/bckt/releases/latest/download/bckt-linux.tar.gz
          tar xzf bckt-linux.tar.gz
          chmod +x bckt
          sudo mv bckt /usr/local/bin/

      - name: Build site
        run: bckt render

      - name: Deploy to GitHub Pages
        uses: peaceiris/actions-gh-pages@v3
        with:
          github_token: ${{ secrets.GITHUB_TOKEN }}
          publish_dir: ./html
```

### Netlify

1. Create a `netlify.toml` in your project root:

```toml
[build]
  command = "bckt render"
  publish = "html"
```

2. Connect your Git repository to Netlify:
   - Log in to Netlify
   - Click "New site from Git"
   - Select your repository
   - Netlify will auto-detect the settings from `netlify.toml`
   - Click "Deploy site"

**Manual deployment:**

```bash
bckt render
netlify deploy --prod --dir=html
```

### Vercel

1. Create `vercel.json` in your project root:

```json
{
  "buildCommand": "bckt render",
  "outputDirectory": "html"
}
```

2. Deploy via Vercel dashboard:
   - Import your Git repository
   - Vercel will detect the configuration
   - Click "Deploy"

**Manual deployment:**

```bash
bckt render
vercel --prod
```

### AWS S3 + CloudFront

1. Build your site:
```bash
bckt render
```

2. Sync to S3:
```bash
aws s3 sync html/ s3://your-bucket-name --delete
```

3. Invalidate CloudFront cache:
```bash
aws cloudfront create-invalidation \
  --distribution-id YOUR_DISTRIBUTION_ID \
  --paths "/*"
```

**Automate with a script** (`deploy.sh`):

```bash
#!/bin/bash
bckt render
aws s3 sync html/ s3://your-bucket-name --delete
aws cloudfront create-invalidation --distribution-id YOUR_DIST_ID --paths "/*"
```

### Self-Hosted Server

Using SSH and rsync:

```bash
bckt render
rsync -avz --delete html/ user@yourserver.com:/var/www/html/
```

Create a deployment script:

```bash
#!/bin/bash
bckt render
rsync -avz --delete \
  -e "ssh -i ~/.ssh/your-key" \
  html/ user@yourserver.com:/var/www/html/
```

### Other Platforms

bckt works with any static hosting provider:

- **Cloudflare Pages**: Connect Git repo, set build to `bckt render`, output to `html/`
- **Render**: Static site with build command `bckt render`, publish directory `html/`
- **DigitalOcean App Platform**: Static site with build `bckt render`, output `html/`
- **Azure Static Web Apps**: Build command `bckt render`, app location `html/`

## Custom Domains

After deploying, configure your custom domain in your hosting provider's dashboard. Make sure your `bckt.yaml` has the correct `base_url`:

```yaml
base_url: "https://yourdomain.com"
```

Then rebuild and redeploy:

```bash
bckt render
# ... deploy using your method
```

## Automation Tips

### Git Hooks

Create `.git/hooks/pre-push` to build before pushing:

```bash
#!/bin/bash
echo "Building site before push..."
bckt render
git add html/
```

Make it executable:
```bash
chmod +x .git/hooks/pre-push
```

### Makefile

Create a `Makefile` for common tasks:

```makefile
.PHONY: build dev clean deploy

build:
	bckt render

dev:
	bckt dev

clean:
	bckt clean

deploy: clean build
	rsync -avz --delete html/ user@server:/var/www/html/
```

Use with:
```bash
make deploy
```

## Troubleshooting

**Build fails after config changes:**
```bash
bckt clean
bckt render --force
```

**Images not showing:**
- Check that images are listed in `attached:` frontmatter
- Verify image paths are correct relative to post directory
- Ensure images exist in the post's directory

**Old content still visible:**
```bash
bckt render --force  # Full rebuild
```

**Changes not appearing:**
- Check that `base_url` in `bckt.yaml` matches your domain
- Clear browser cache
- Invalidate CDN cache if using one

## Next Steps

Your blog is now published! Learn how to maintain and grow it over time.

Continue to: [Maintaining Your Blog](05-maintenance.md)
