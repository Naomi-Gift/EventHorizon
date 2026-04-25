const Trigger = require('../models/trigger.model');
const logger = require('../config/logger');
const AppError = require('../utils/appError');
const asyncHandler = require('../utils/asyncHandler');

exports.createTrigger = asyncHandler(async (req, res) => {
    logger.info('Creating new trigger', {
        contractId: req.body.contractId,
        eventName: req.body.eventName,
        userAgent: req.get('User-Agent'),
        ip: req.ip,
    });

    const trigger = new Trigger(req.body);
    await trigger.save();

    logger.info('Trigger created successfully', {
        triggerId: trigger._id,
        contractId: trigger.contractId,
        eventName: trigger.eventName,
        isActive: trigger.isActive,
    });

    res.status(201).json({
        success: true,
        data: trigger,
    });
});

exports.getTriggers = asyncHandler(async (req, res) => {
    logger.debug('Fetching all triggers', { ip: req.ip });

    const triggers = await Trigger.find();

    logger.info('Triggers fetched successfully', {
        count: triggers.length,
        ip: req.ip,
    });

    res.json({
        success: true,
        data: triggers,
    });
});

exports.deleteTrigger = asyncHandler(async (req, res) => {
    logger.info('Deleting trigger', {
        triggerId: req.params.id,
        ip: req.ip,
    });

    const trigger = await Trigger.findByIdAndDelete(req.params.id);

    if (!trigger) {
        logger.warn('Trigger not found for deletion', {
            triggerId: req.params.id,
            ip: req.ip,
        });

        throw new AppError('Trigger not found', 404);
    }

    logger.info('Trigger deleted successfully', {
        triggerId: req.params.id,
        contractId: trigger.contractId,
        eventName: trigger.eventName,
        ip: req.ip,
    });

    res.status(204).send();
});

exports.updateTrigger = asyncHandler(async (req, res) => {
    logger.info('Updating trigger', {
        triggerId: req.params.id,
        ip: req.ip,
    });

    const trigger = await Trigger.findByIdAndUpdate(
        req.params.id,
        req.body,
        { new: true, runValidators: true }
    );

    if (!trigger) {
        logger.warn('Trigger not found for update', {
            triggerId: req.params.id,
            ip: req.ip,
        });

        throw new AppError('Trigger not found', 404);
    }

    logger.info('Trigger updated successfully', {
        triggerId: req.params.id,
        contractId: trigger.contractId,
        eventName: trigger.eventName,
        batchingEnabled: trigger.batchingConfig?.enabled,
        ip: req.ip,
    });

    res.json({
        success: true,
        data: trigger,
    });
});

exports.regenerateWebhookSecret = asyncHandler(async (req, res) => {
    logger.info('Regenerating webhook secret', {
        triggerId: req.params.id,
        ip: req.ip,
    });

    const trigger = await Trigger.findById(req.params.id);

    if (!trigger) {
        logger.warn('Trigger not found for webhook secret regeneration', {
            triggerId: req.params.id,
            ip: req.ip,
        });

        throw new AppError('Trigger not found', 404);
    }

    if (trigger.actionType !== 'webhook') {
        logger.warn('Attempted to regenerate webhook secret for non-webhook trigger', {
            triggerId: req.params.id,
            actionType: trigger.actionType,
            ip: req.ip,
        });

        throw new AppError('Webhook secret regeneration is only available for webhook triggers', 400);
    }

    const oldSecret = trigger.webhookSecret;
    trigger.webhookSecret = require('crypto').randomBytes(32).toString('hex');
    await trigger.save();

    logger.info('Webhook secret regenerated successfully', {
        triggerId: req.params.id,
        contractId: trigger.contractId,
        oldSecretPrefix: oldSecret.substring(0, 8),
        newSecretPrefix: trigger.webhookSecret.substring(0, 8),
        ip: req.ip,
    });

    res.json({
        success: true,
        message: 'Webhook secret regenerated successfully',
        data: {
            triggerId: trigger._id,
            webhookSecret: trigger.webhookSecret,
        },
    });
});
