const telegramService = require('../src/services/telegram.service');
require('dotenv').config();

const testTelegramNotification = async () => {
    // These should be set in your .env file for actual testing
    const botToken = process.env.TELEGRAM_BOT_TOKEN;
    const chatId = process.env.TELEGRAM_CHAT_ID;

    if (!botToken || !chatId) {
        console.warn('⚠️  Skipping actual API test: TELEGRAM_BOT_TOKEN or TELEGRAM_CHAT_ID is not set in .env');
        console.log('Testing MarkdownV2 escaping logic...');
        const rawText = 'Hello! *this* is a [test] (link) with . dots and ! marks.';
        const escaped = telegramService.escapeMarkdownV2(rawText);
        console.log(`Original: ${rawText}`);
        console.log(`Escaped:  ${escaped}`);
        
        if (escaped.includes('\\.')) {
            console.log('✅ Escaping logic looks correct.');
        } else {
            console.error('❌ Escaping logic failed.');
        }
        return;
    }

    try {
        console.log(`Sending test message to chat ${chatId}...`);
        const text = '🚀 *EventHorizon Notification Service Test*\n\nStatus: _Connected successfully_';
        
        const response = await telegramService.sendTelegramMessage(botToken, chatId, text);
        
        if (response.ok) {
            console.log('✅ Test message sent successfully!');
        } else {
            console.error('❌ Failed to send message:', response.description);
        }
    } catch (error) {
        console.error('❌ Error during test:', error.message);
    }
};

testTelegramNotification().catch(console.error);
