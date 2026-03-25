const slackService = require('../src/services/slack.service');
require('dotenv').config();

const testSlackNotification = async () => {
    // These should be set in your .env file for actual testing
    const webhookUrl = process.env.SLACK_WEBHOOK_URL;

    if (!webhookUrl) {
        console.warn('⚠️  Skipping actual API test: SLACK_WEBHOOK_URL is not set in .env');
        console.log('Testing Block Kit payload generation...');
        
        const mockEvent = {
            topic: ['ContractUpgraded'],
            severity: 'warning',
            contractId: 'CBAQ43...',
            payload: {
                old_version: '1.0.0',
                new_version: '1.1.0'
            }
        };

        const payload = slackService.buildAlertBlocks(mockEvent);
        console.log(JSON.stringify(payload, null, 2));
        
        if (payload.blocks && payload.blocks.length === 4) {
            console.log('✅ Block Kit generation looks correct.');
        } else {
            console.error('❌ Block Kit generation failed or returned unexpected blocks count.');
        }
        return;
    }

    try {
        console.log(`Sending test payload to Slack...`);
        
        const mockEvent = {
            topic: ['SystemAlert'],
            severity: 'info',
            contractId: 'TestEnv',
            payload: 'This is a test notification from EventHorizon.'
        };

        const payload = slackService.buildAlertBlocks(mockEvent);
        
        const response = await slackService.sendSlackAlert(webhookUrl, payload);
        
        if (response.success) {
            console.log('✅ Test webhook triggered successfully!');
        } else {
            console.error('❌ Failed to trigger webhook:', response.message);
        }
    } catch (error) {
        console.error('❌ Error during test:', error.message);
    }
};

testSlackNotification().catch(console.error);
