const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');

// Configuration
const BASE_NAME = process.argv[2] || 'wechat-proxy';
const COUNT = parseInt(process.argv[3] || '5');
const START_INDEX = parseInt(process.argv[4] || '1');

console.log(`Preparing to deploy ${COUNT} workers starting from index ${START_INDEX}...`);
console.log(`Base Name: ${BASE_NAME}`);

if (!fs.existsSync('worker.js')) {
    console.error('Error: worker.js not found in current directory.');
    process.exit(1);
}

// Ensure Wrangler is authenticated
try {
    console.log('Checking Wrangler authentication...');
    execSync('npx wrangler whoami', { stdio: 'inherit' });
} catch (e) {
    console.error('Please login to Wrangler first using: npx wrangler login');
    process.exit(1);
}

// Deploy Loop
const deployedUrls = [];

for (let i = 0; i < COUNT; i++) {
    const idx = START_INDEX + i;
    const workerName = `${BASE_NAME}-${idx}`;
    console.log(`\n[${i + 1}/${COUNT}] Deploying ${workerName}...`);

    try {
        // Run wrangler deploy
        // We use --name to override the name in wrangler.toml (or creating a temp one)
        // Since we don't have a wrangler.toml, we pass everything via CLI
        const cmd = `npx wrangler deploy worker.js --name ${workerName} --compatibility-date 2024-01-01 --no-bundle`;

        // Wrangler outputs the URL at the end. We capture stdout.
        const output = execSync(cmd, { encoding: 'utf8' });
        console.log(output);

        // Extract URL from output (regex)
        const match = output.match(/https:\/\/[a-zA-Z0-9-]+\.[a-zA-Z0-9-]+\.workers\.dev/);
        if (match) {
            deployedUrls.push(match[0]);
            console.log(`✓ Success: ${match[0]}`);
        }
    } catch (e) {
        console.error(`✗ Failed to deploy ${workerName}:`, e.message);
    }
}

// Summary
console.log('\n\n=== Deployment Summary ===');
console.log('Deployed URLs (Copy these to your Exporter configuration):');
console.log(deployedUrls.join('\n'));

// Save to file
fs.writeFileSync('deployed_proxies.txt', deployedUrls.join('\n'));
console.log('\nSaved list to deployed_proxies.txt');
