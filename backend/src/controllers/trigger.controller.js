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
