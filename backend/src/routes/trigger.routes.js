const express = require('express');
const router = express.Router();
const triggerController = require('../controllers/trigger.controller');
const auditMiddleware = require('../middleware/audit.middleware');
const {
    validateBody,
    validationSchemas,
} = require('../middleware/validation.middleware');

/**
 * @openapi
 * /api/triggers:
 *   post:
 *     summary: Create a trigger
 *     description: Register a new Soroban event trigger and the action to execute when it fires.
 *     tags:
 *       - Triggers
 *     requestBody:
 *       required: true
 *       content:
 *         application/json:
 *           schema:
 *             $ref: '#/components/schemas/TriggerInput'
 *     responses:
 *       201:
 *         description: Trigger created successfully.
 *         content:
 *           application/json:
 *             schema:
 *               $ref: '#/components/schemas/Trigger'
 *       400:
 *         description: Invalid trigger payload.
 *         content:
 *           application/json:
 *             schema:
 *               $ref: '#/components/schemas/ErrorResponse'
 *   get:
 *     summary: List triggers
 *     description: Return all configured triggers.
 *     tags:
 *       - Triggers
 *     responses:
 *       200:
 *         description: List of triggers.
 *         content:
 *           application/json:
 *             schema:
 *               type: array
 *               items:
 *                 $ref: '#/components/schemas/Trigger'
 *       500:
 *         description: Failed to fetch triggers.
 *         content:
 *           application/json:
 *             schema:
 *               $ref: '#/components/schemas/ErrorResponse'
 */
router.post(
    '/',
    auditMiddleware.auditCreate(),
    validateBody(validationSchemas.triggerCreate),
    triggerController.createTrigger
);
router.get('/', triggerController.getTriggers);

/**
 * @openapi
 * /api/triggers/{id}:
 *   delete:
 *     summary: Delete a trigger
 *     description: Remove an existing trigger by its MongoDB identifier.
 *     tags:
 *       - Triggers
 *     parameters:
 *       - in: path
 *         name: id
 *         required: true
 *         description: Trigger identifier.
 *         schema:
 *           type: string
 *     responses:
 *       204:
 *         description: Trigger deleted successfully.
 *       500:
 *         description: Failed to delete the trigger.
 *         content:
 *           application/json:
 *             schema:
 *               $ref: '#/components/schemas/ErrorResponse'
 */
router.delete('/:id',
    auditMiddleware.auditDelete(),
    triggerController.deleteTrigger
);

/**
 * @openapi
 * /api/triggers/{id}:
 *   put:
 *     summary: Update a trigger
 *     description: Update an existing trigger configuration including batching settings.
 *     tags:
 *       - Triggers
 *     parameters:
 *       - in: path
 *         name: id
 *         required: true
 *         description: Trigger identifier.
 *         schema:
 *           type: string
 *     requestBody:
 *       required: true
 *       content:
 *         application/json:
 *           schema:
 *             $ref: '#/components/schemas/TriggerInput'
 *     responses:
 *       200:
 *         description: Trigger updated successfully.
 *         content:
 *           application/json:
 *             schema:
 *               $ref: '#/components/schemas/Trigger'
 *       404:
 *         description: Trigger not found.
 *         content:
 *           application/json:
 *             schema:
 *               $ref: '#/components/schemas/ErrorResponse'
 *       400:
 *         description: Invalid trigger payload.
 *         content:
 *           application/json:
 *             schema:
 *               $ref: '#/components/schemas/ErrorResponse'
 */
router.put('/:id',
    auditMiddleware.auditUpdate(),
    triggerController.updateTrigger
);

/**
 * @openapi
 * /api/triggers/{id}/regenerate-secret:
 *   post:
 *     summary: Regenerate webhook secret
 *     description: Generate a new webhook secret for HMAC signature verification. Only available for webhook triggers.
 *     tags:
 *       - Triggers
 *     parameters:
 *       - in: path
 *         name: id
 *         required: true
 *         description: Trigger identifier.
 *         schema:
 *           type: string
 *     responses:
 *       200:
 *         description: Webhook secret regenerated successfully.
 *         content:
 *           application/json:
 *             schema:
 *               type: object
 *               properties:
 *                 success:
 *                   type: boolean
 *                   example: true
 *                 message:
 *                   type: string
 *                   example: "Webhook secret regenerated successfully"
 *                 data:
 *                   type: object
 *                   properties:
 *                     triggerId:
 *                       type: string
 *                       example: "507f1f77bcf86cd799439011"
 *                     webhookSecret:
 *                       type: string
 *                       example: "a1b2c3d4e5f678901234567890123456789012345678901234567890123456789012"
 *       404:
 *         description: Trigger not found.
 *         content:
 *           application/json:
 *             schema:
 *               $ref: '#/components/schemas/ErrorResponse'
 *       400:
 *         description: Not a webhook trigger.
 *         content:
 *           application/json:
 *             schema:
 *               $ref: '#/components/schemas/ErrorResponse'
 */
router.post('/:id/regenerate-secret',
    auditMiddleware.auditUpdate(),
    triggerController.regenerateWebhookSecret
);

module.exports = router;
