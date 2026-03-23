const Joi = require('joi');

const validationSchemas = {
    triggerCreate: Joi.object({
        contractId: Joi.string().trim().required(),
        eventName: Joi.string().trim().required(),
        actionType: Joi.string().valid('webhook', 'discord', 'email').default('webhook'),
        actionUrl: Joi.string().trim().uri().required(),
        isActive: Joi.boolean().default(true),
        lastPolledLedger: Joi.number().integer().min(0).default(0),
    }),
    authCredentials: Joi.object({
        email: Joi.string().trim().email().required(),
        password: Joi.string().min(8).required(),
    }),
};

const mapValidationErrors = (details) =>
    details.map((detail) => ({
        field: detail.path.join('.'),
        message: detail.message,
    }));

const validateRequest = (schema, source = 'body') => (req, res, next) => {
    const { error, value } = schema.validate(req[source], {
        abortEarly: false,
        stripUnknown: source === 'body',
        convert: true,
    });

    if (error) {
        return res.status(400).json({
            success: false,
            error: 'Validation failed',
            details: mapValidationErrors(error.details),
        });
    }

    req[source] = value;
    return next();
};

const validateBody = (schema) => validateRequest(schema, 'body');

module.exports = {
    validationSchemas,
    validateRequest,
    validateBody,
};