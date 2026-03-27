const { Worker } = require('bullmq');
const Redis = require('ioredis');
const axios = require('axios');
const { sendEventNotification } = require('../services/email.service');
const { sendDiscordNotification } = require('../services/discord.service');
const telegramService = require('../services/telegram.service');
const logger = require('../config/logger');

const REDIS_HOST = process.env.REDIS_HOST || 'localhost';
const REDIS_PORT = process.env.REDIS_PORT || 6379;
const REDIS_PASSWORD = process.env.REDIS_PASSWORD || undefined;
const WORKER_CONCURRENCY = Number(process.env.WORKER_CONCURRENCY || 5);

const connection = new Redis({
    host: REDIS_HOST,
    port: REDIS_PORT,
    password: REDIS_PASSWORD,
    lazyConnect: true,
    maxRetriesPerRequest: null,
});

/**
 * Execute the action based on the trigger type
 */
async function executeAction(job) {
    const { trigger, eventPayload } = job.data;
    const { actionType, actionUrl, contractId, eventName } = trigger;

    logger.info('Processing action job', {
        jobId: job.id,
        actionType,
        contractId,
        eventName,
        attempt: job.attemptsMade + 1,
    });

    switch (actionType) {
        case 'email': {
            return await sendEventNotification({
                trigger,
                payload: eventPayload,
            });
        }

        case 'discord': {
            if (!actionUrl) {
                throw new Error('Missing actionUrl for Discord trigger');
            }

            const discordPayload = {
                embeds: [{
                    title: `Event: ${eventName}`,
                    description: `Contract: ${contractId}`,
                    fields: [
                        {
                            name: 'Payload',
                            value: `\`\`\`json\n${JSON.stringify(eventPayload, null, 2).slice(0, 1000)}\n\`\`\``,
                        },
                    ],
                    color: 0x5865F2,
                    timestamp: new Date().toISOString(),
                }],
            };

            return await sendDiscordNotification(actionUrl, discordPayload);
        }

        case 'telegram': {
            const { botToken, chatId } = trigger;
            if (!botToken || !chatId) {
                throw new Error('Missing botToken or chatId for Telegram trigger');
            }

            const message = `🔔 *Event Triggered*\n\n` +
                `*Event:* ${telegramService.escapeMarkdownV2(eventName)}\n` +
                `*Contract:* \`${contractId}\`\n\n` +
                `*Payload:*\n\`\`\`\n${JSON.stringify(eventPayload, null, 2)}\n\`\`\``;

            return await telegramService.sendTelegramMessage(botToken, chatId, message);
        }

        case 'webhook': {
            if (!actionUrl) {
                throw new Error('Missing actionUrl for webhook trigger');
            }

            return await axios.post(actionUrl, {
                contractId,
                eventName,
                payload: eventPayload,
            });
        }

        default:
            throw new Error(`Unsupported action type: ${actionType}`);
    }
}

/**
 * Create and start the BullMQ worker
 */
function createWorker() {
    const worker = new Worker(
        'action-queue',
        async (job) => {
            try {
                const result = await executeAction(job);
                
                logger.info('Action job completed successfully', {
                    jobId: job.id,
                    actionType: job.data.trigger.actionType,
                    result: result,
                });

                return result;
            } catch (error) {
                logger.error('Action job failed', {
                    jobId: job.id,
                    actionType: job.data.trigger.actionType,
                    error: error.message,
                    stack: error.stack,
                    attempt: job.attemptsMade + 1,
                });

                throw error;
            }
        },
        {
            connection,
            concurrency: WORKER_CONCURRENCY,
            limiter: {
                max: 10,
                duration: 1000,
            },
        }
    );

    worker.on('completed', (job) => {
        logger.info('Job completed', {
            jobId: job.id,
            actionType: job.data.trigger.actionType,
        });
    });

    worker.on('failed', (job, err) => {
        logger.error('Job failed', {
            jobId: job?.id,
            actionType: job?.data?.trigger?.actionType,
            error: err.message,
            attemptsRemaining: job ? job.opts.attempts - job.attemptsMade : 0,
        });
    });

    worker.on('error', (err) => {
        logger.error('Worker error', {
            error: err.message,
            stack: err.stack,
        });
    });

    logger.info('BullMQ worker started', {
        concurrency: WORKER_CONCURRENCY,
        redisHost: REDIS_HOST,
        redisPort: REDIS_PORT,
    });

    return worker;
}

module.exports = {
    createWorker,
    connection,
};
