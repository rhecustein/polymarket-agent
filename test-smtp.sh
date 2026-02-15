#!/bin/bash
# ══════════════════════════════════════════════
# SMTP Email Test Script
# Tests email configuration using agent code
# ══════════════════════════════════════════════

set -e

echo "══════════════════════════════════════════════"
echo "  SMTP Email Configuration Test"
echo "══════════════════════════════════════════════"
echo ""

# Load .env
if [ ! -f ".env" ]; then
    echo "❌ .env file not found!"
    exit 1
fi

source .env 2>/dev/null || true

echo "[*] Configuration:"
echo "    SMTP Host: ${SMTP_HOST}"
echo "    SMTP Port: ${SMTP_PORT}"
echo "    SMTP User: ${SMTP_USER}"
echo "    From:      ${ALERT_FROM}"
echo "    To:        ${ALERT_TO}"
echo ""

# Test with swaks (if available)
if command -v swaks &> /dev/null; then
    echo "[*] Testing with swaks..."
    swaks --to "${ALERT_TO}" \
          --from "${SMTP_USER}" \
          --server "${SMTP_HOST}:${SMTP_PORT}" \
          --auth LOGIN \
          --auth-user "${SMTP_USER}" \
          --auth-password "${SMTP_PASS}" \
          --tls \
          --header "Subject: 🧪 Polymarket Agent - SMTP Test" \
          --body "Email configuration test successful! Timestamp: $(date)"
    
    echo ""
    echo "✅ Email sent! Check inbox: ${ALERT_TO}"
else
    echo "⚠️  swaks not installed. Using curl..."
    
    # Create email file
    cat > /tmp/email.txt << EMAILEOF
From: ${ALERT_FROM}
To: ${ALERT_TO}
Subject: 🧪 Polymarket Agent - SMTP Test
Content-Type: text/html; charset=UTF-8

<!DOCTYPE html>
<html>
<body style="font-family: Arial; background: #f5f5f5; padding: 20px;">
  <div style="max-width: 600px; margin: 0 auto; background: white; padding: 30px; border-radius: 10px;">
    <h2 style="color: #2ed573;">✅ SMTP Test Successful!</h2>
    <p>Your email configuration is working correctly.</p>
    <p><strong>Timestamp:</strong> $(date)</p>
    <hr>
    <p style="color: #666; font-size: 12px;">Polymarket AI Agent | Gemini-Only Mode</p>
  </div>
</body>
</html>
EMAILEOF

    echo "[*] Sending via curl..."
    curl --url "smtp://${SMTP_HOST}:${SMTP_PORT}" \
         --ssl-reqd \
         --mail-from "${SMTP_USER}" \
         --mail-rcpt "${ALERT_TO}" \
         --user "${SMTP_USER}:${SMTP_PASS}" \
         --upload-file /tmp/email.txt \
         -v
    
    rm -f /tmp/email.txt
    
    echo ""
    echo "✅ Email sent! Check inbox: ${ALERT_TO}"
fi

echo ""
echo "══════════════════════════════════════════════"
echo "  ✅ SMTP Test Complete"
echo "══════════════════════════════════════════════"
