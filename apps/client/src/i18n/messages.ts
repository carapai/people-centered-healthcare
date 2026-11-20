import { defineMessages } from 'react-intl'

// The app's strings, as ICU messages (client-application.md § Design system). Ids are
// `app.<feature>.<name>`; the design system's are `ds.*`.
export const messages = defineMessages({
  appName: { id: 'app.shell.name', defaultMessage: 'Health Platform', description: 'Product name on the home screen' },
  modeCentral: { id: 'app.status.mode.central', defaultMessage: 'Central', description: 'Status area: the client talks to the central server' },
  modeEdge: { id: 'app.status.mode.edge', defaultMessage: 'Facility edge', description: 'Status area: the client talks to its facility edge server' },
  modeTauri: { id: 'app.status.mode.tauri', defaultMessage: 'Local runtime', description: 'Status area: the desktop app runs its own local runtime' },
  syncPending: { id: 'app.status.syncPending', defaultMessage: 'Sync status pending', description: 'Status area: sync detail is not available yet' },
  portalTitle: { id: 'app.portal.title', defaultMessage: 'Your health portal', description: 'Patient portal heading' },
  portalIntro: {
    id: 'app.portal.intro',
    defaultMessage: 'Sign in with your phone number. We send you a code by text message each time.',
    description: 'Patient portal: before signing in',
  },
  portalSignIn: { id: 'app.portal.signIn', defaultMessage: 'Sign in with your phone', description: 'Patient portal: sign-in button' },
  portalUnavailable: { id: 'app.portal.unavailable', defaultMessage: 'The patient portal is not available here.', description: 'Patient portal: not configured on this node' },
  portalFailed: { id: 'app.portal.failed', defaultMessage: 'Your registration could not be loaded. Please try again.', description: 'Patient portal: loading failed' },
  portalRegisterHint: {
    id: 'app.portal.registerHint',
    defaultMessage: 'Register yourself: enter your details as they appear on your ID.',
    description: 'Patient portal: registration form guidance',
  },
  portalSubmit: { id: 'app.portal.submit', defaultMessage: 'Register', description: 'Patient portal: submit registration' },
  portalSubmitFailed: { id: 'app.portal.submitFailed', defaultMessage: 'Your registration was not sent. Check your details and try again.', description: 'Patient portal: submission failed' },
  portalInReview: {
    id: 'app.portal.inReview',
    defaultMessage: 'Thank you. We are checking whether you are already registered at a facility. Bring your ID to your next visit so staff can confirm it is you.',
    description: 'Patient portal: registration waiting for staff review',
  },
  portalRegistered: {
    id: 'app.portal.registered',
    defaultMessage: 'You are registered. You can see the details you gave us. To see your health records, show your ID at a facility.',
    description: 'Patient portal: registered, below level3',
  },
  portalCheckAgain: { id: 'app.portal.checkAgain', defaultMessage: 'Check again', description: 'Patient portal: refresh the registration status' },
  portalLinked: { id: 'app.portal.linked', defaultMessage: 'Your account is linked to your health record.', description: 'Patient portal: linked at level3' },
  signInPhone: { id: 'app.signIn.phone', defaultMessage: 'Phone number', description: 'Patient sign-in: phone field' },
  signInPhoneHelp: { id: 'app.signIn.phoneHelp', defaultMessage: 'For example 0772 123456', description: 'Patient sign-in: phone example' },
  signInSendCode: { id: 'app.signIn.sendCode', defaultMessage: 'Send me a code', description: 'Patient sign-in: request a code' },
  signInCode: { id: 'app.signIn.code', defaultMessage: 'Code from the text message', description: 'Patient sign-in: code field' },
  signInCodeSent: { id: 'app.signIn.codeSent', defaultMessage: 'We sent a code to your phone. It works for 5 minutes.', description: 'Patient sign-in: code sent' },
  signInNewCode: { id: 'app.signIn.newCode', defaultMessage: 'Send a new code', description: 'Patient sign-in: ask for another code' },
  signInPhoneInvalid: { id: 'app.signIn.phoneInvalid', defaultMessage: 'Enter your phone number, for example 0772 123456.', description: 'Patient sign-in: unreadable phone' },
  signInWait: { id: 'app.signIn.wait', defaultMessage: 'A code was sent recently. Wait a minute, then ask for a new one.', description: 'Patient sign-in: rate limited' },
  signInCodeRefused: { id: 'app.signIn.codeRefused', defaultMessage: 'That code is not right. Check it, or ask for a new one.', description: 'Patient sign-in: wrong code' },
  signInCodeExpired: { id: 'app.signIn.codeExpired', defaultMessage: 'That code can no longer be used. Ask for a new one.', description: 'Patient sign-in: expired or spent code' },
  ptOpen: { id: 'app.patient.open', defaultMessage: 'Open patient record', description: 'Link to the patient view' },
  ptFailed: { id: 'app.patient.failed', defaultMessage: 'The patient could not be loaded.', description: 'Patient view: loading failed' },
  cgTitle: { id: 'app.caregivers.title', defaultMessage: 'Caregivers', description: 'Patient view: caregivers section' },
  cgFailed: { id: 'app.caregivers.failed', defaultMessage: 'Caregivers could not be loaded.', description: 'Caregivers: loading failed' },
  cgNone: { id: 'app.caregivers.none', defaultMessage: 'No caregivers are recorded.', description: 'Caregivers: none' },
  cgGuardian: { id: 'app.caregivers.guardian', defaultMessage: 'Guardian', description: 'Caregiver authority: parent or guardian of a minor' },
  cgDelegated: { id: 'app.caregivers.delegated', defaultMessage: 'Delegated by the patient', description: 'Caregiver authority: chosen by an adult patient' },
  cgUntil: { id: 'app.caregivers.until', defaultMessage: 'until', description: 'Caregiver access: followed by the last date' },
  cgStatusActive: { id: 'app.caregivers.statusActive', defaultMessage: 'Active', description: 'Caregiver access status' },
  cgStatusRevoked: { id: 'app.caregivers.statusRevoked', defaultMessage: 'Revoked', description: 'Caregiver access status' },
  cgStatusExpired: { id: 'app.caregivers.statusExpired', defaultMessage: 'Expired', description: 'Caregiver access status' },
  cgStatusUnverified: { id: 'app.caregivers.statusUnverified', defaultMessage: 'Not verified', description: 'Caregiver access status' },
  cgScopeView: { id: 'app.caregivers.scopeView', defaultMessage: 'View records', description: 'Caregiver scope' },
  cgScopeBook: { id: 'app.caregivers.scopeBook', defaultMessage: 'Book appointments', description: 'Caregiver scope' },
  cgScopeRefills: { id: 'app.caregivers.scopeRefills', defaultMessage: 'Request refills', description: 'Caregiver scope' },
  cgScopeMessages: { id: 'app.caregivers.scopeMessages', defaultMessage: 'Receive messages', description: 'Caregiver scope' },
  cgAdolescentWithheld: {
    id: 'app.caregivers.adolescentWithheld',
    defaultMessage: 'From age 12, sensitive records are not shown to this guardian unless the young person consents.',
    description: 'Caregivers: adolescent confidentiality applies',
  },
  cgAdolescentConsents: { id: 'app.caregivers.adolescentConsents', defaultMessage: 'The young person consents to this guardian seeing their sensitive records.', description: 'Caregivers: adolescent consent recorded' },
  cgRecordConsent: { id: 'app.caregivers.recordConsent', defaultMessage: "Record the young person's consent", description: 'Caregivers action' },
  cgWithdrawConsent: { id: 'app.caregivers.withdrawConsent', defaultMessage: "Withdraw the young person's consent", description: 'Caregivers action' },
  cgRevoke: { id: 'app.caregivers.revoke', defaultMessage: 'Revoke access', description: 'Caregivers action' },
  cgAdd: { id: 'app.caregivers.add', defaultMessage: 'Add a caregiver', description: 'Caregivers: add form heading' },
  cgAddHint: {
    id: 'app.caregivers.addHint',
    defaultMessage: "Check the caregiver's documents in person. They must have signed in to the patient portal once with this phone.",
    description: 'Caregivers: add form guidance',
  },
  cgPhone: { id: 'app.caregivers.phone', defaultMessage: "Caregiver's phone", description: 'Caregivers field' },
  cgRelationship: { id: 'app.caregivers.relationship', defaultMessage: 'Relationship', description: 'Caregivers field' },
  cgRelMother: { id: 'app.caregivers.relMother', defaultMessage: 'Mother', description: 'Relationship' },
  cgRelFather: { id: 'app.caregivers.relFather', defaultMessage: 'Father', description: 'Relationship' },
  cgRelGuardian: { id: 'app.caregivers.relGuardian', defaultMessage: 'Guardian', description: 'Relationship' },
  cgRelFamily: { id: 'app.caregivers.relFamily', defaultMessage: 'Other family member', description: 'Relationship' },
  cgAuthority: { id: 'app.caregivers.authority', defaultMessage: 'Authority', description: 'Caregivers field' },
  cgScopes: { id: 'app.caregivers.scopes', defaultMessage: 'The caregiver may', description: 'Caregivers field: scopes' },
  cgExpires: { id: 'app.caregivers.expires', defaultMessage: 'Last day of access', description: 'Caregivers field' },
  cgDocument: { id: 'app.caregivers.document', defaultMessage: 'Document checked', description: 'Caregivers field' },
  cgRecord: { id: 'app.caregivers.record', defaultMessage: 'Record caregiver', description: 'Caregivers action' },
  cgRecordFailed: { id: 'app.caregivers.recordFailed', defaultMessage: 'The caregiver was not recorded.', description: 'Caregivers: record failed' },
  portalDependents: { id: 'app.portal.dependents', defaultMessage: 'People you care for', description: 'Patient portal: dependents' },
  portalDependentUnnamed: { id: 'app.portal.dependentUnnamed', defaultMessage: 'A person you care for', description: 'Patient portal: dependent without a name' },
  portalConfidential: {
    id: 'app.portal.confidential',
    defaultMessage: 'From age 12, some of their records stay private to them unless they agree to share them with you.',
    description: 'Patient portal: adolescent confidentiality',
  },
  portalRefreshDependents: { id: 'app.portal.refreshDependents', defaultMessage: 'Refresh the people you care for', description: 'Patient portal: refresh dependents' },
  stTitle: { id: 'app.stock.title', defaultMessage: 'Stock', description: 'Stock page heading and link' },
  stStore: { id: 'app.stock.store', defaultMessage: 'Store', description: 'Stock: the store' },
  stReceive: { id: 'app.stock.receive', defaultMessage: 'Receive goods', description: 'Stock: goods received note' },
  stFailed: { id: 'app.stock.failed', defaultMessage: 'Stock could not be loaded.', description: 'Stock: loading failed' },
  stEmpty: { id: 'app.stock.empty', defaultMessage: 'This store has no stock recorded.', description: 'Stock: none' },
  stOnHandCaption: { id: 'app.stock.onHandCaption', defaultMessage: 'Stock on hand', description: 'Stock: on-hand table caption' },
  stProduct: { id: 'app.stock.product', defaultMessage: 'Product', description: 'Stock column/field' },
  stBatch: { id: 'app.stock.batch', defaultMessage: 'Batch', description: 'Stock column/field' },
  stExpiry: { id: 'app.stock.expiry', defaultMessage: 'Expiry', description: 'Stock column/field' },
  stStatus: { id: 'app.stock.status', defaultMessage: 'Status', description: 'Stock column' },
  stOnHand: { id: 'app.stock.onHand', defaultMessage: 'On hand', description: 'Stock column' },
  stUnitCost: { id: 'app.stock.unitCost', defaultMessage: 'Unit cost', description: 'Stock column/field' },
  stReorder: { id: 'app.stock.reorder', defaultMessage: 'Reorder suggestions', description: 'Stock: reorder section' },
  stReorderHint: {
    id: 'app.stock.reorderHint',
    defaultMessage: 'From the last 30 days of use: order up to three months of stock when one month or less is left.',
    description: 'Stock: how reorder suggestions are made',
  },
  stMonthly: { id: 'app.stock.monthly', defaultMessage: 'Used a month', description: 'Reorder column' },
  stMin: { id: 'app.stock.min', defaultMessage: 'Minimum', description: 'Reorder column' },
  stMax: { id: 'app.stock.max', defaultMessage: 'Maximum', description: 'Reorder column' },
  stOrder: { id: 'app.stock.order', defaultMessage: 'Suggested order', description: 'Reorder column' },
  stSupplier: { id: 'app.stock.supplier', defaultMessage: 'Supplier', description: 'Goods received field' },
  stDelivery: { id: 'app.stock.delivery', defaultMessage: 'Delivery or invoice number', description: 'Goods received field' },
  stLine: { id: 'app.stock.line', defaultMessage: 'Line', description: 'Goods received: followed by the line number' },
  stBarcode: { id: 'app.stock.barcode', defaultMessage: 'Barcode (optional)', description: 'Goods received field' },
  stBarcodeHelp: { id: 'app.stock.barcodeHelp', defaultMessage: 'Scan a GS1 DataMatrix to fill the batch and expiry.', description: 'Goods received: barcode help' },
  stReadBarcode: { id: 'app.stock.readBarcode', defaultMessage: 'Read barcode', description: 'Goods received action' },
  stBarcodeFailed: { id: 'app.stock.barcodeFailed', defaultMessage: 'That barcode could not be read. Enter the batch and expiry by hand.', description: 'Goods received: barcode failed' },
  stQuantity: { id: 'app.stock.quantity', defaultMessage: 'Quantity (base units)', description: 'Goods received field' },
  stPackSize: { id: 'app.stock.packSize', defaultMessage: 'Pack size', description: 'Goods received field' },
  stAddLine: { id: 'app.stock.addLine', defaultMessage: 'Add another line', description: 'Goods received action' },
  stReceiveSubmit: { id: 'app.stock.receiveSubmit', defaultMessage: 'Receive', description: 'Goods received action' },
  stReceiveFailed: { id: 'app.stock.receiveFailed', defaultMessage: 'The goods were not received.', description: 'Goods received: failed' },
  stSerials: { id: 'app.stock.serials', defaultMessage: 'Serial numbers (equipment only)', description: 'Goods received: serial numbers of an equipment line' },
  stSerialsHelp: {
    id: 'app.stock.serialsHelp',
    defaultMessage: 'For a product tracked as equipment: one serial number or asset tag per unit, one per line or separated by commas, as many as the quantity. Leave batch and expiry empty.',
    description: 'Goods received: help for serial numbers',
  },
  stReceived: { id: 'app.stock.received', defaultMessage: 'Received. The stock is on hand.', description: 'Goods received: success' },
  rqTitle: { id: 'app.requisitions.title', defaultMessage: 'Requisitions', description: 'Requisitions page heading and link' },
  rqThisStore: { id: 'app.requisitions.thisStore', defaultMessage: 'Your store', description: 'Requisitions: the store working on the page' },
  rqNew: { id: 'app.requisitions.new', defaultMessage: 'Ask for stock', description: 'Requisitions: new requisition heading' },
  rqFrom: { id: 'app.requisitions.from', defaultMessage: 'From store', description: 'Requisitions: the store asked' },
  rqSend: { id: 'app.requisitions.send', defaultMessage: 'Send requisition', description: 'Requisitions action' },
  rqFailed: { id: 'app.requisitions.failed', defaultMessage: 'The requisition was not sent.', description: 'Requisitions: send failed' },
  rqList: { id: 'app.requisitions.list', defaultMessage: 'Requisitions to and from this store', description: 'Requisitions: list heading' },
  rqLoadFailed: { id: 'app.requisitions.loadFailed', defaultMessage: 'Requisitions could not be loaded.', description: 'Requisitions: load failed' },
  rqNone: { id: 'app.requisitions.none', defaultMessage: 'No requisitions.', description: 'Requisitions: none' },
  rqRequested: { id: 'app.requisitions.requested', defaultMessage: 'Waiting for approval', description: 'Requisition status' },
  rqApproved: { id: 'app.requisitions.approved', defaultMessage: 'Approved', description: 'Requisition status' },
  rqFulfilled: { id: 'app.requisitions.fulfilled', defaultMessage: 'Sent', description: 'Requisition status: fulfilled, in transit' },
  rqRejected: { id: 'app.requisitions.rejected', defaultMessage: 'Refused', description: 'Requisition status' },
  rqApprove: { id: 'app.requisitions.approve', defaultMessage: 'Approve', description: 'Requisitions action' },
  rqReject: { id: 'app.requisitions.reject', defaultMessage: 'Refuse', description: 'Requisitions action' },
  rqBatchFor: { id: 'app.requisitions.batchFor', defaultMessage: 'Batch to send for', description: 'Requisitions: followed by the product' },
  rqChooseBatch: { id: 'app.requisitions.chooseBatch', defaultMessage: 'Choose a batch', description: 'Requisitions: no batch chosen' },
  rqFulfil: { id: 'app.requisitions.fulfil', defaultMessage: 'Send the stock', description: 'Requisitions action: fulfil' },
  rqFulfilFailed: { id: 'app.requisitions.fulfilFailed', defaultMessage: 'The stock was not sent.', description: 'Requisitions: fulfil failed' },
  tlTitle: { id: 'app.till.title', defaultMessage: 'Till', description: 'Cashier till page heading and link' },
  tlTill: { id: 'app.till.till', defaultMessage: 'Till', description: 'Till field' },
  tlFailed: { id: 'app.till.failed', defaultMessage: 'The till could not be loaded.', description: 'Till: load failed' },
  tlClosed: { id: 'app.till.closed', defaultMessage: 'No shift is open at this till.', description: 'Till: closed' },
  tlFloat: { id: 'app.till.float', defaultMessage: 'Opening float', description: 'Till field' },
  tlOpen: { id: 'app.till.open', defaultMessage: 'Open shift', description: 'Till action' },
  tlShift: { id: 'app.till.shift', defaultMessage: 'Shift', description: 'Till: followed by the shift id' },
  tlExpected: { id: 'app.till.expected', defaultMessage: 'cash expected', description: 'Till: followed by the amount' },
  tlTake: { id: 'app.till.take', defaultMessage: 'Take a payment', description: 'Till: payment form heading' },
  tlMethod: { id: 'app.till.method', defaultMessage: 'Method', description: 'Payment field' },
  tlCash: { id: 'app.till.cash', defaultMessage: 'Cash', description: 'Payment method' },
  tlMtn: { id: 'app.till.mtn', defaultMessage: 'MTN Mobile Money', description: 'Payment method' },
  tlAirtel: { id: 'app.till.airtel', defaultMessage: 'Airtel Money', description: 'Payment method' },
  tlCard: { id: 'app.till.card', defaultMessage: 'Card', description: 'Payment method' },
  tlBank: { id: 'app.till.bank', defaultMessage: 'Bank transfer', description: 'Payment method' },
  tlReported: { id: 'app.till.reported', defaultMessage: 'Reported by the customer (SMS)', description: 'Payment method' },
  tlAmount: { id: 'app.till.amount', defaultMessage: 'Amount', description: 'Payment field' },
  tlPayer: { id: 'app.till.payer', defaultMessage: 'Paid for (account or patient)', description: 'Payment field' },
  tlReference: { id: 'app.till.reference', defaultMessage: 'Provider reference', description: 'Payment field' },
  tlReferenceHelp: { id: 'app.till.referenceHelp', defaultMessage: 'The transaction ID from the provider, or from the customer\'s SMS.', description: 'Payment field help' },
  tlRecord: { id: 'app.till.record', defaultMessage: 'Record payment', description: 'Till action' },
  tlTakeFailed: { id: 'app.till.takeFailed', defaultMessage: 'The payment was not recorded.', description: 'Till: payment failed' },
  tlReceipts: { id: 'app.till.receipts', defaultMessage: 'Receipts this shift', description: 'Till: receipts heading' },
  tlRefresh: { id: 'app.till.refresh', defaultMessage: 'Refresh receipts', description: 'Till action' },
  tlProvisional: { id: 'app.till.provisional', defaultMessage: 'Provisional', description: 'Receipt status: waiting for the provider' },
  tlConfirmed: { id: 'app.till.confirmed', defaultMessage: 'Confirmed', description: 'Receipt status' },
  tlVoid: { id: 'app.till.void', defaultMessage: 'Void', description: 'Receipt status' },
  tlClose: { id: 'app.till.close', defaultMessage: 'Close the shift', description: 'Till: close heading' },
  tlCounted: { id: 'app.till.counted', defaultMessage: 'Cash counted', description: 'Till field' },
  tlCloseSubmit: { id: 'app.till.closeSubmit', defaultMessage: 'Close shift', description: 'Till action' },
  tlSettled: { id: 'app.till.settled', defaultMessage: 'Shift settled.', description: 'Till: settled' },
  tlAwaiting: { id: 'app.till.awaiting', defaultMessage: 'Shift closed; a supervisor must approve the variance of', description: 'Till: variance waiting for approval, followed by the amount' },
  tlSuperviseTitle: { id: 'app.till.superviseTitle', defaultMessage: 'Supervise payments', description: 'Supervisor page heading and link' },
  tlStatement: { id: 'app.till.statement', defaultMessage: 'Provider statement line', description: 'Supervisor: statement heading' },
  tlReconcile: { id: 'app.till.reconcile', defaultMessage: 'Reconcile', description: 'Supervisor action' },
  tlReconciled: { id: 'app.till.reconciled', defaultMessage: 'Confirmed:', description: 'Supervisor: payments confirmed, followed by the count' },
  tlExceptions: { id: 'app.till.exceptions', defaultMessage: 'exceptions:', description: 'Supervisor: followed by the count' },
  tlApprove: { id: 'app.till.approve', defaultMessage: 'Approve a shift variance', description: 'Supervisor: approve heading' },
  tlApproveSubmit: { id: 'app.till.approveSubmit', defaultMessage: 'Approve variance', description: 'Supervisor action' },
  signInTitle: { id: 'app.signIn.title', defaultMessage: 'Sign in', description: 'Sign-in page heading' },
  signInUsername: { id: 'app.signIn.username', defaultMessage: 'Username', description: 'Sign-in page: username field label' },
  signInPassword: { id: 'app.signIn.password', defaultMessage: 'Password', description: 'Sign-in page: password field label' },
  signInSubmit: { id: 'app.signIn.submit', defaultMessage: 'Sign in', description: 'Sign-in page: submit button' },
  signInRefused: {
    id: 'app.signIn.refused',
    defaultMessage: 'The username or password is incorrect, or the account is locked.',
    description: 'Sign-in page: one message for every refusal, so it does not reveal whether an account exists',
  },
  signInFacility: { id: 'app.signIn.facility', defaultMessage: 'Facility code', description: 'Sign-in page: facility field label' },
  signInFacilityHelp: {
    id: 'app.signIn.facilityHelp',
    defaultMessage: 'Needed only if you work at more than one facility.',
    description: 'Sign-in page: helper text under the facility field',
  },
  signInChooseFacility: {
    id: 'app.signIn.chooseFacility',
    defaultMessage: 'You work at more than one facility. Enter the code of the facility you are signing in for, then sign in again.',
    description: 'Sign-in page: shown when the user holds roles at several facilities and named none',
  },
  signInChangeTitle: { id: 'app.signIn.changeTitle', defaultMessage: 'Choose your own password', description: 'Sign-in page: heading when the user must replace the password an admin set' },
  signInChangeIntro: {
    id: 'app.signIn.changeIntro',
    defaultMessage: 'Your account was created with a temporary password. Enter it, then choose a password only you know.',
    description: 'Sign-in page: why the user must change their password before signing in',
  },
  signInCurrentPassword: { id: 'app.signIn.currentPassword', defaultMessage: 'Temporary password', description: 'Sign-in page: the password the admin set' },
  signInNewPassword: { id: 'app.signIn.newPassword', defaultMessage: 'New password', description: 'Sign-in page: the user\'s own new password' },
  signInConfirmPassword: { id: 'app.signIn.confirmPassword', defaultMessage: 'New password again', description: 'Sign-in page: the new password, repeated' },
  signInNewPasswordHelp: {
    id: 'app.signIn.newPasswordHelp',
    defaultMessage: 'At least 8 characters. A few unrelated words make a strong one.',
    description: 'Sign-in page: the password policy on an enrolled device',
  },
  signInChangeSubmit: { id: 'app.signIn.changeSubmit', defaultMessage: 'Set password and sign in', description: 'Sign-in page: change the password, then sign in' },
  signInMismatch: { id: 'app.signIn.mismatch', defaultMessage: 'The new passwords do not match.', description: 'Sign-in page: the two new passwords differ' },
  signInRejected: {
    id: 'app.signIn.rejected',
    defaultMessage: 'That password cannot be used: it is too short or too common. Choose another.',
    description: 'Sign-in page: the new password failed the policy or the breached-password list',
  },
  signInChangeFailed: { id: 'app.signIn.changeFailed', defaultMessage: 'The password could not be changed. Please try again.', description: 'Sign-in page: the change failed for another reason' },
  lockTitle: { id: 'app.lock.title', defaultMessage: 'This device is locked', description: 'Lock screen heading' },
  lockIntro: {
    id: 'app.lock.intro',
    defaultMessage: 'It locked after 15 minutes without use. Enter your PIN to carry on, or the username and PIN of another user signed in on this device to switch to them.',
    description: 'Lock screen: what happened and how to unlock or switch',
  },
  lockUsername: { id: 'app.lock.username', defaultMessage: 'Username', description: 'Lock screen: username field' },
  lockPin: { id: 'app.lock.pin', defaultMessage: 'PIN', description: 'Lock screen: PIN field' },
  lockSubmit: { id: 'app.lock.submit', defaultMessage: 'Unlock', description: 'Lock screen: submit button' },
  lockRefused: { id: 'app.lock.refused', defaultMessage: 'That username and PIN did not unlock this device.', description: 'Lock screen: wrong PIN or user' },
  lockPassword: { id: 'app.lock.password', defaultMessage: 'Sign in with a password instead', description: 'Lock screen: fall back to a full sign-in' },
  pinTitle: { id: 'app.pin.title', defaultMessage: 'PIN for this device', description: 'Home: heading of the PIN form' },
  pinField: { id: 'app.pin.field', defaultMessage: 'New 6-digit PIN', description: 'Home: PIN field' },
  pinSubmit: { id: 'app.pin.submit', defaultMessage: 'Set PIN', description: 'Home: PIN submit button' },
  pinSaved: { id: 'app.pin.saved', defaultMessage: 'Your PIN is set for this device.', description: 'Home: the PIN was set' },
  pinFailed: { id: 'app.pin.failed', defaultMessage: 'The PIN could not be set. Use exactly 6 digits.', description: 'Home: the PIN was refused' },
  signInStart: { id: 'app.signIn.start', defaultMessage: 'Sign in', description: 'Home: button that starts sign-in' },
  signOut: { id: 'app.signIn.signOut', defaultMessage: 'Sign out', description: 'Home: button that signs the user out' },
  signedIn: { id: 'app.signIn.signedIn', defaultMessage: 'Signed in', description: 'Home: shown once the session is confirmed by the server' },
  signInCompleting: { id: 'app.signIn.completing', defaultMessage: 'Signing you in…', description: 'Callback page while the code is redeemed' },
  signInFailed: { id: 'app.signIn.failed', defaultMessage: 'Sign-in could not be completed. Please sign in again.', description: 'Callback page when sign-in fails' },
  bgAlertsTitle: { id: 'app.breakGlass.alertsTitle', defaultMessage: 'Break-glass alerts', description: 'Heading of the facility in-charge alert list' },
  bgAlertsEmpty: { id: 'app.breakGlass.alertsEmpty', defaultMessage: 'No break-glass alerts to review.', description: 'Alert list: nothing pending' },
  bgReviewsTitle: { id: 'app.breakGlass.reviewsTitle', defaultMessage: 'Break-glass reviews', description: 'Heading of the privacy officer review queue' },
  bgReviewsEmpty: { id: 'app.breakGlass.reviewsEmpty', defaultMessage: 'No break-glass access awaits review.', description: 'Review queue: nothing pending' },
  bgForbidden: { id: 'app.breakGlass.forbidden', defaultMessage: 'Your role does not include this list.', description: 'Break-glass pages: the user lacks the permission' },
  bgFailed: { id: 'app.breakGlass.failed', defaultMessage: 'The list could not be loaded. Try again.', description: 'Break-glass pages: the request failed' },
  bgLoading: { id: 'app.breakGlass.loading', defaultMessage: 'Loading…', description: 'Break-glass pages: the list is loading' },
  bgColWhen: { id: 'app.breakGlass.col.when', defaultMessage: 'When', description: 'Column: when glass was broken' },
  bgColPatient: { id: 'app.breakGlass.col.patient', defaultMessage: 'Patient', description: 'Column: the patient whose records were opened' },
  bgColUser: { id: 'app.breakGlass.col.user', defaultMessage: 'User', description: 'Column: who broke glass' },
  bgColFacility: { id: 'app.breakGlass.col.facility', defaultMessage: 'Facility', description: 'Column: the facility' },
  bgColReason: { id: 'app.breakGlass.col.reason', defaultMessage: 'Justification', description: "Column: the user's justification" },
  bgColUntil: { id: 'app.breakGlass.col.until', defaultMessage: 'Open until', description: 'Column: when the grant ends' },
  bgColDue: { id: 'app.breakGlass.col.due', defaultMessage: 'Review due', description: 'Column: when the privacy review is due' },
  bgColAction: { id: 'app.breakGlass.col.action', defaultMessage: 'Action', description: 'Column: the row action' },
  bgOverdue: { id: 'app.breakGlass.overdue', defaultMessage: 'Overdue', description: 'Review queue: the review is past due' },
  bgAcknowledge: { id: 'app.breakGlass.acknowledge', defaultMessage: 'Acknowledge', description: 'Alert list: button marking an alert seen' },
  bgDecideHeading: { id: 'app.breakGlass.decideHeading', defaultMessage: 'Review this access', description: 'Review form heading' },
  bgSelectHint: { id: 'app.breakGlass.selectHint', defaultMessage: 'Select a row (Enter) to review it.', description: 'Review queue: how to start a review' },
  bgNote: { id: 'app.breakGlass.note', defaultMessage: 'Note', description: 'Review form: note field label' },
  bgNoteHelp: { id: 'app.breakGlass.noteHelp', defaultMessage: 'Required when the access was not appropriate.', description: 'Review form: note helper text' },
  bgAppropriate: { id: 'app.breakGlass.appropriate', defaultMessage: 'Appropriate', description: 'Review form: the access was appropriate' },
  bgInappropriate: { id: 'app.breakGlass.inappropriate', defaultMessage: 'Not appropriate', description: 'Review form: the access was not appropriate' },
  bgDecideFailed: { id: 'app.breakGlass.decideFailed', defaultMessage: 'The decision was not saved. A note is required for an access that was not appropriate.', description: 'Review form: the server refused the decision' },
  regTitle: { id: 'app.registration.title', defaultMessage: 'Register a patient', description: 'Reception registration page heading' },
  regFamily: { id: 'app.registration.family', defaultMessage: 'Family name', description: 'Registration field' },
  regGiven: { id: 'app.registration.given', defaultMessage: 'Given names', description: 'Registration field' },
  regSex: { id: 'app.registration.sex', defaultMessage: 'Sex', description: 'Registration field' },
  regSexChoose: { id: 'app.registration.sexChoose', defaultMessage: 'Choose…', description: 'Registration: no sex chosen yet' },
  regFemale: { id: 'app.registration.female', defaultMessage: 'Female', description: 'Registration: sex option' },
  regMale: { id: 'app.registration.male', defaultMessage: 'Male', description: 'Registration: sex option' },
  regOther: { id: 'app.registration.other', defaultMessage: 'Other', description: 'Registration: sex option' },
  regUnknown: { id: 'app.registration.unknown', defaultMessage: 'Unknown', description: 'Registration: sex option' },
  regBirthDate: { id: 'app.registration.birthDate', defaultMessage: 'Date of birth', description: 'Registration field' },
  regAge: { id: 'app.registration.age', defaultMessage: 'Age in years (if the date is not known)', description: 'Registration field' },
  regPhone: { id: 'app.registration.phone', defaultMessage: 'Phone', description: 'Registration field' },
  regNin: { id: 'app.registration.nin', defaultMessage: 'National ID (NIN)', description: 'Registration field' },
  regDistrict: { id: 'app.registration.district', defaultMessage: 'District', description: 'Registration field' },
  regVillage: { id: 'app.registration.village', defaultMessage: 'Village or parish', description: 'Registration field' },
  regFind: { id: 'app.registration.find', defaultMessage: 'Check for this patient', description: 'Registration: look for existing records before registering' },
  regCandidates: { id: 'app.registration.candidates', defaultMessage: 'Did you mean one of these patients?', description: 'Registration: heading of the candidate list' },
  regCandidatesHint: { id: 'app.registration.candidatesHint', defaultMessage: 'Use the arrow keys to move through the list and Enter to choose a patient.', description: 'Registration: keyboard hint for the candidate list' },
  regColName: { id: 'app.registration.col.name', defaultMessage: 'Name', description: 'Candidate column' },
  regColBirth: { id: 'app.registration.col.birth', defaultMessage: 'Born', description: 'Candidate column' },
  regColSex: { id: 'app.registration.col.sex', defaultMessage: 'Sex', description: 'Candidate column' },
  regColMrn: { id: 'app.registration.col.mrn', defaultMessage: 'MRN', description: 'Candidate column' },
  regColMatch: { id: 'app.registration.col.match', defaultMessage: 'Match', description: 'Candidate column: the match grade' },
  regNoneOfThese: { id: 'app.registration.noneOfThese', defaultMessage: 'I have checked: this patient is none of these', description: 'Registration: confirmation before creating a new patient despite candidates' },
  regRegisterNew: { id: 'app.registration.registerNew', defaultMessage: 'Register as a new patient', description: 'Registration: create the patient' },
  regChosen: { id: 'app.registration.chosen', defaultMessage: 'Existing patient chosen:', description: 'Registration: the clerk chose a candidate' },
  regInsufficient: { id: 'app.registration.insufficient', defaultMessage: 'There is too little to match on. Search the registry by name, birth year and village or parish first.', description: 'Registration: matching needs more identifying data' },
  regSearch: { id: 'app.registration.search', defaultMessage: 'Search the registry', description: 'Registration: manual search button' },
  regSearchFailed: { id: 'app.registration.searchFailed', defaultMessage: 'The registry search is not available here.', description: 'Registration: manual search failed' },
  regNoResults: { id: 'app.registration.noResults', defaultMessage: 'No patients found.', description: 'Registration: manual search found nothing' },
  regRegistered: { id: 'app.registration.registered', defaultMessage: 'Registered. MRN:', description: 'Registration: success' },
  regOutcomeLinked: { id: 'app.registration.outcome.linked', defaultMessage: 'Linked to the existing registry record.', description: 'Registration outcome' },
  regOutcomeQueued: { id: 'app.registration.outcome.queued', defaultMessage: 'A possible duplicate was sent for review; the record can be used for care now.', description: 'Registration outcome' },
  regOutcomeNew: { id: 'app.registration.outcome.new', defaultMessage: 'A new registry identity was created.', description: 'Registration outcome' },
  regFailed: { id: 'app.registration.failed', defaultMessage: 'The registration could not be completed. Check the details and try again.', description: 'Registration: the request failed' },
  regAnother: { id: 'app.registration.another', defaultMessage: 'Register another patient', description: 'Registration: start over' },
  rrTitle: { id: 'app.registryReview.title', defaultMessage: 'Registry review', description: 'Registry review queue heading' },
  rrEmpty: { id: 'app.registryReview.empty', defaultMessage: 'Nothing awaits review.', description: 'Registry review: queue empty' },
  rrHint: { id: 'app.registryReview.hint', defaultMessage: 'Click an item to open it, or move through the queue with the arrow keys and press Enter.', description: 'Registry review: keyboard hint' },
  rrColKind: { id: 'app.registryReview.col.kind', defaultMessage: 'Item', description: 'Registry review column' },
  rrColQueued: { id: 'app.registryReview.col.queued', defaultMessage: 'Queued', description: 'Registry review column' },
  rrColSummary: { id: 'app.registryReview.col.summary', defaultMessage: 'About', description: 'Registry review column' },
  rrKindMatch: { id: 'app.registryReview.kind.match', defaultMessage: 'Possible duplicate', description: 'Registry review: a potential match' },
  rrKindSurvivorship: { id: 'app.registryReview.kind.survivorship', defaultMessage: 'Disagreeing value', description: 'Registry review: a survivorship item' },
  rrOverdue: { id: 'app.registryReview.overdue', defaultMessage: 'Overdue', description: 'Registry review: older than 7 days' },
  rrForbidden: { id: 'app.registryReview.forbidden', defaultMessage: 'Your role does not include registry review.', description: 'Registry review: no permission' },
  rrFailed: { id: 'app.registryReview.failed', defaultMessage: 'The queue could not be loaded. Try again.', description: 'Registry review: load failed' },
  rrDecideFailed: { id: 'app.registryReview.decideFailed', defaultMessage: 'The decision was not saved. The item may already be decided.', description: 'Registry review: decision failed' },
  rrField: { id: 'app.registryReview.field', defaultMessage: 'Field', description: 'Comparison column' },
  rrNewRecord: { id: 'app.registryReview.newRecord', defaultMessage: 'New registration', description: 'Comparison column: the newly registered record' },
  rrExisting: { id: 'app.registryReview.existing', defaultMessage: 'Existing record', description: 'Comparison column: the existing record' },
  rrAgrees: { id: 'app.registryReview.agrees', defaultMessage: 'Agrees', description: 'Comparison: the values agree' },
  rrDiffers: { id: 'app.registryReview.differs', defaultMessage: 'Differs', description: 'Comparison: the values differ' },
  rrScore: { id: 'app.registryReview.score', defaultMessage: 'Match weight:', description: 'Registry review: the match score label' },
  rrSamePerson: { id: 'app.registryReview.samePerson', defaultMessage: 'Same person', description: 'Registry review action' },
  rrDifferentPeople: { id: 'app.registryReview.differentPeople', defaultMessage: 'Different people', description: 'Registry review action' },
  rrCurrent: { id: 'app.registryReview.current', defaultMessage: 'Current value', description: 'Survivorship: the surviving value' },
  rrIncoming: { id: 'app.registryReview.incoming', defaultMessage: 'Incoming value', description: 'Survivorship: the value that did not survive' },
  rrTrust: { id: 'app.registryReview.trust', defaultMessage: 'Trust', description: 'Survivorship: trust level label' },
  rrSource: { id: 'app.registryReview.source', defaultMessage: 'From', description: 'Survivorship: which source record' },
  rrKeepCurrent: { id: 'app.registryReview.keepCurrent', defaultMessage: 'Keep current', description: 'Registry review action' },
  rrTakeIncoming: { id: 'app.registryReview.takeIncoming', defaultMessage: 'Take incoming', description: 'Registry review action' },
  rrKindRegistration: { id: 'app.registryReview.kindRegistration', defaultMessage: 'Portal registration', description: 'Registry review item kind: a self-registration that matched existing patients' },
  rrRegistrationHint: {
    id: 'app.registryReview.registrationHint',
    defaultMessage: 'A patient registered through the portal and resembles existing patients. Link the account only after checking their ID document in person.',
    description: 'Registry review: guidance for a portal registration',
  },
  rrSubmitted: { id: 'app.registryReview.submitted', defaultMessage: 'Submitted by the patient', description: 'Comparison column: what the patient submitted' },
  rrDocument: { id: 'app.registryReview.document', defaultMessage: 'ID document checked in person', description: 'Registry review: the document the reviewer checked' },
  rrDocNationalId: { id: 'app.registryReview.docNationalId', defaultMessage: 'National ID', description: 'ID document' },
  rrDocPassport: { id: 'app.registryReview.docPassport', defaultMessage: 'Passport', description: 'ID document' },
  rrDocDrivingLicence: { id: 'app.registryReview.docDrivingLicence', defaultMessage: 'Driving licence', description: 'ID document' },
  rrDocRefugeeId: { id: 'app.registryReview.docRefugeeId', defaultMessage: 'Refugee ID', description: 'ID document' },
  rrChooseDocument: { id: 'app.registryReview.chooseDocument', defaultMessage: 'Choose the document you checked', description: 'Registry review: document not chosen yet' },
  rrLinkPatient: { id: 'app.registryReview.linkPatient', defaultMessage: 'ID checked: this is the patient', description: 'Registry review action: link the portal account to this candidate' },
  rrCreatePatient: { id: 'app.registryReview.createPatient', defaultMessage: 'None of these: create a new patient', description: 'Registry review action: a new patient for the portal account' },
})

/** The edge status page's messages' ICU arguments, so `formatMessage` checks the values passed. */
type EdgeStatusValues = {
  esTitle: {}
  esLoading: {}
  esFailed: {}
  esForbidden: {}
  esNotEdge: {}
  esAlerts: {}
  esNoAlerts: {}
  esAlertDiskFull: { subject: string }
  esAlertBackupsOff: {}
  esAlertBackupFailed: {}
  esAlertBackupStale: {}
  esAlertBackupTestFailed: {}
  esAlertOnBattery: {}
  esAlertLowBattery: {}
  esAlertUpsUnreachable: {}
  esAlertTaskRestarts: { subject: string }
  esAlertDiskUnencrypted: { subject: string }
  esAlertTpmProtectorMissing: { subject: string }
  esAlertEncryptionSuspended: { subject: string }
  esAlertTpmNotSealed: { subject: string }
  esAlertEncryptionUnknown: { subject: string }
  esAlertTpmMissing: {}
  esAlertTpmUnknown: {}
  esAlertSecureBootOff: {}
  esAlertSecureBootUnknown: {}
  esAlertEncryptionExempt: {}
  esAlertNotProvisioned: {}
  esAlertIdentityInvalid: {}
  esAlertCertificateRenewalFailed: {}
  esAlertCertificateExpiring: {}
  esAlertIntermediateExpiring: {}
  esAlertAddressChanged: {}
  esCertificate: {}
  esCertServing: {}
  esCertNotProvisioned: {}
  esCertInvalid: {}
  esCertState: {}
  esCertNames: {}
  esCertExpires: {}
  esCertRenews: {}
  esIntermediateExpires: {}
  esCertProblem: {}
  esPairing: {}
  esPairingHint: {}
  esPairingQr: { node: string }
  esPairingUrl: {}
  esPairingPin: {}
  esPairingCode: {}
  esPairingNotProvisioned: {}
  esPairingUnavailable: { reason: string }
  esDevices: {}
  esPendingRequests: {}
  esNoPending: {}
  esRequested: { label: string; expires: string }
  esApprove: {}
  esApproveCode: {}
  esDeviceName: {}
  esDeviceLocation: {}
  esApproved: { name: string }
  esEnrolledDevices: {}
  esNoDevices: {}
  esDeviceLine: { name: string; location: string; label: string }
  esDeviceRevokedAt: { at: string }
  esRevoke: { name: string }
  esConfirmRevoke: { name: string }
  esCancel: {}
  esActionFailed: { reason: string }
  esSystemVolume: {}
  esSystemDisk: {}
  esEncryption: {}
  esTpm: {}
  esSecureBoot: {}
  esYes: {}
  esNo: {}
  esExemption: {}
  esProtTpm: {}
  esProtSuspended: {}
  esProtNoTpm: {}
  esProtNotSealed: {}
  esProtUnencrypted: {}
  esProtUnknown: { reason: string }
  esNode: {}
  esNodeId: {}
  esVersion: {}
  esStarted: {}
  esChecked: {}
  esStorage: {}
  esStoreSize: {}
  esDataVolume: {}
  esDataDisk: {}
  esBackupDisk: {}
  esBackupVolume: {}
  esVolumeUse: { percent: number; free: string }
  esBackups: {}
  esBackupsOff: {}
  esLastBackup: {}
  esLastFailure: {}
  esLastTest: {}
  esTestPassed: { at: string }
  esTestFailed: { at: string }
  esPower: {}
  esUps: {}
  esUpsOff: {}
  esPowerOnline: {}
  esPowerOnBattery: {}
  esPowerLowBattery: {}
  esPowerUnreachable: {}
  esTelemetry: {}
  esBuffered: {}
  esDropped: {}
  esLastShipped: {}
  esShippingOff: {}
  esShippingFailing: { at: string }
  esTaskRestarts: { task: string }
  esNever: {}
  esUnknown: {}
  esPending: { indicators: string }
}

// The edge admin status page's strings (features/edge-status).
export const edgeStatusMessages = defineMessages<EdgeStatusValues>({
  esTitle: { id: 'app.edgeStatus.title', defaultMessage: 'Edge status', description: 'Edge admin status page heading' },
  esLoading: { id: 'app.edgeStatus.loading', defaultMessage: 'Checking the edge…', description: 'Edge status: loading' },
  esFailed: { id: 'app.edgeStatus.failed', defaultMessage: 'The edge status could not be loaded. Please try again.', description: 'Edge status: loading failed' },
  esForbidden: { id: 'app.edgeStatus.forbidden', defaultMessage: 'Open this page on the edge server itself: the status is shown only there until edge sign-in is available.', description: 'Edge status: refused for a remote browser' },
  esNotEdge: { id: 'app.edgeStatus.notEdge', defaultMessage: 'This page is available on a facility edge only.', description: 'Edge status: not served by central' },
  esAlerts: { id: 'app.edgeStatus.alerts', defaultMessage: 'Alerts', description: 'Edge status: alerts section' },
  esNoAlerts: { id: 'app.edgeStatus.noAlerts', defaultMessage: 'No alerts.', description: 'Edge status: nothing to act on' },
  esAlertDiskFull: { id: 'app.edgeStatus.alert.diskFull', defaultMessage: 'The {subject} is more than 80% full.', description: 'Alert: a volume is nearly full; subject is the volume' },
  esAlertBackupsOff: { id: 'app.edgeStatus.alert.backupsOff', defaultMessage: 'Backups are not configured on this node.', description: 'Alert: no backup directory' },
  esAlertBackupFailed: { id: 'app.edgeStatus.alert.backupFailed', defaultMessage: 'The last backup failed.', description: 'Alert' },
  esAlertBackupStale: { id: 'app.edgeStatus.alert.backupStale', defaultMessage: 'There has been no good backup for more than two backup intervals.', description: 'Alert' },
  esAlertBackupTestFailed: { id: 'app.edgeStatus.alert.backupTestFailed', defaultMessage: 'The last test restore failed.', description: 'Alert' },
  esAlertOnBattery: { id: 'app.edgeStatus.alert.onBattery', defaultMessage: 'The UPS is on battery.', description: 'Alert' },
  esAlertLowBattery: { id: 'app.edgeStatus.alert.lowBattery', defaultMessage: 'The UPS battery is low: the node is stopping.', description: 'Alert' },
  esAlertUpsUnreachable: { id: 'app.edgeStatus.alert.upsUnreachable', defaultMessage: 'The UPS cannot be reached.', description: 'Alert' },
  esAlertTaskRestarts: { id: 'app.edgeStatus.alert.taskRestarts', defaultMessage: 'The {subject} task keeps restarting.', description: 'Alert: a background task fails repeatedly; subject is its name' },
  esAlertDiskUnencrypted: { id: 'app.edgeStatus.alert.diskUnencrypted', defaultMessage: 'The {subject} is not encrypted.', description: 'Alert: full-disk encryption is off; subject is the volume' },
  esAlertTpmProtectorMissing: { id: 'app.edgeStatus.alert.tpmProtectorMissing', defaultMessage: 'The {subject} has no TPM unlock, so the edge cannot restart unattended.', description: 'Alert: encrypted without a TPM protector; subject is the volume' },
  esAlertEncryptionSuspended: { id: 'app.edgeStatus.alert.encryptionSuspended', defaultMessage: 'Encryption of the {subject} is suspended.', description: 'Alert: BitLocker protection suspended; subject is the volume' },
  esAlertTpmNotSealed: { id: 'app.edgeStatus.alert.tpmNotSealed', defaultMessage: 'The TPM unlock of the {subject} is not tied to Secure Boot.', description: 'Alert: the LUKS TPM token is not sealed to PCR 7; subject is the volume' },
  esAlertEncryptionUnknown: { id: 'app.edgeStatus.alert.encryptionUnknown', defaultMessage: 'The encryption of the {subject} could not be checked.', description: 'Alert: the encryption probe failed; subject is the volume' },
  esAlertTpmMissing: { id: 'app.edgeStatus.alert.tpmMissing', defaultMessage: 'This machine has no TPM 2.0.', description: 'Alert' },
  esAlertTpmUnknown: { id: 'app.edgeStatus.alert.tpmUnknown', defaultMessage: 'The TPM could not be checked.', description: 'Alert' },
  esAlertSecureBootOff: { id: 'app.edgeStatus.alert.secureBootOff', defaultMessage: 'Secure Boot is off.', description: 'Alert' },
  esAlertSecureBootUnknown: { id: 'app.edgeStatus.alert.secureBootUnknown', defaultMessage: 'Secure Boot could not be checked.', description: 'Alert' },
  esAlertEncryptionExempt: { id: 'app.edgeStatus.alert.encryptionExempt', defaultMessage: 'This facility is exempt from full-disk encryption.', description: 'Alert: a standing country-configuration exemption' },
  esAlertNotProvisioned: { id: 'app.edgeStatus.alert.notProvisioned', defaultMessage: 'This edge is not provisioned: it serves this machine only, without TLS.', description: 'Alert: no facility intermediate installed' },
  esAlertIdentityInvalid: { id: 'app.edgeStatus.alert.identityInvalid', defaultMessage: 'The edge cannot serve its TLS certificate: it serves this machine only.', description: 'Alert: the installed identity cannot be used' },
  esAlertCertificateRenewalFailed: { id: 'app.edgeStatus.alert.certificateRenewalFailed', defaultMessage: 'The TLS certificate could not be renewed.', description: 'Alert: issuing the server certificate failed' },
  esAlertCertificateExpiring: { id: 'app.edgeStatus.alert.certificateExpiring', defaultMessage: 'The TLS certificate expires in under 30 days.', description: 'Alert: the server certificate is close to expiry' },
  esAlertIntermediateExpiring: { id: 'app.edgeStatus.alert.intermediateExpiring', defaultMessage: 'The facility intermediate is close to expiry: re-provision this edge.', description: 'Alert: the facility intermediate has under 400 days left' },
  esAlertAddressChanged: { id: 'app.edgeStatus.alert.addressChanged', defaultMessage: "The edge's network address changed; its certificate was reissued.", description: 'Alert: the LAN address changed in the last day' },
  esCertificate: { id: 'app.edgeStatus.certificate', defaultMessage: 'TLS certificate', description: 'Edge status section heading' },
  esCertServing: { id: 'app.edgeStatus.certificate.serving', defaultMessage: 'Serving', description: 'TLS certificate state' },
  esCertNotProvisioned: { id: 'app.edgeStatus.certificate.notProvisioned', defaultMessage: 'Not provisioned', description: 'TLS certificate state' },
  esCertInvalid: { id: 'app.edgeStatus.certificate.invalid', defaultMessage: 'Cannot be served', description: 'TLS certificate state' },
  esCertState: { id: 'app.edgeStatus.certificate.state', defaultMessage: 'State', description: 'Edge status: the TLS certificate state' },
  esCertNames: { id: 'app.edgeStatus.certificate.names', defaultMessage: 'Names', description: "Edge status: the certificate's DNS names and addresses" },
  esCertExpires: { id: 'app.edgeStatus.certificate.expires', defaultMessage: 'Expires', description: 'Edge status: when the certificate expires' },
  esCertRenews: { id: 'app.edgeStatus.certificate.renews', defaultMessage: 'Renews', description: 'Edge status: when the certificate is renewed' },
  esIntermediateExpires: { id: 'app.edgeStatus.certificate.intermediateExpires', defaultMessage: 'Facility intermediate expires', description: 'Edge status: when the facility intermediate expires' },
  esCertProblem: { id: 'app.edgeStatus.certificate.problem', defaultMessage: 'Last renewal failed', description: 'Edge status: why the last renewal failed' },
  esPairing: { id: 'app.edgeStatus.pairing', defaultMessage: 'Pair a device', description: 'Edge status section heading: pairing a LAN device' },
  esPairingHint: { id: 'app.edgeStatus.pairing.hint', defaultMessage: 'Scan this code with the app on the new device, or enter the pairing code. Then approve the device below when it asks to enroll.', description: 'Edge status: how to pair a device' },
  esPairingQr: { id: 'app.edgeStatus.pairing.qr', defaultMessage: 'QR code for pairing a device with {node}', description: 'Accessible name of the pairing QR code; node is the node ID' },
  esPairingUrl: { id: 'app.edgeStatus.pairing.url', defaultMessage: 'Address', description: 'Edge status: the URL devices reach the edge by' },
  esPairingPin: { id: 'app.edgeStatus.pairing.pin', defaultMessage: 'Key fingerprint', description: "Edge status: the facility intermediate's SPKI fingerprint" },
  esPairingCode: { id: 'app.edgeStatus.pairing.code', defaultMessage: 'Pairing code', description: 'Edge status: the pairing URI as text' },
  esPairingNotProvisioned: { id: 'app.edgeStatus.pairing.notProvisioned', defaultMessage: 'Devices can pair once this edge is provisioned.', description: 'Edge status: no pairing before provisioning' },
  esDevices: { id: 'app.edgeStatus.devices', defaultMessage: 'Devices', description: 'Edge status section heading: enrolled devices and requests' },
  esPendingRequests: { id: 'app.edgeStatus.devices.pending', defaultMessage: 'Waiting for approval', description: 'Edge status: heading of the pending enrollment requests' },
  esNoPending: { id: 'app.edgeStatus.devices.noPending', defaultMessage: 'No device is asking to enroll.', description: 'Edge status: no pending requests' },
  esRequested: { id: 'app.edgeStatus.devices.requested', defaultMessage: '{label}, until {expires}', description: 'Edge status: a pending request; label is the device, expires a time' },
  esApprove: { id: 'app.edgeStatus.devices.approve', defaultMessage: 'Approve a device', description: 'Edge status: approval form heading and button' },
  esApproveCode: { id: 'app.edgeStatus.devices.code', defaultMessage: 'Pairing code shown on the device', description: 'Edge status: approval form field' },
  esDeviceName: { id: 'app.edgeStatus.devices.name', defaultMessage: 'Device name', description: 'Edge status: approval form field' },
  esDeviceLocation: { id: 'app.edgeStatus.devices.location', defaultMessage: 'Location or ward', description: 'Edge status: approval form field' },
  esApproved: { id: 'app.edgeStatus.devices.approved', defaultMessage: '{name} is enrolled.', description: 'Edge status: a device was approved' },
  esEnrolledDevices: { id: 'app.edgeStatus.devices.enrolled', defaultMessage: 'Enrolled devices', description: 'Edge status: heading of the device list' },
  esNoDevices: { id: 'app.edgeStatus.devices.none', defaultMessage: 'No device is enrolled yet.', description: 'Edge status: empty device list' },
  esDeviceLine: { id: 'app.edgeStatus.devices.line', defaultMessage: '{name}, {location} ({label})', description: 'Edge status: one enrolled device' },
  esDeviceRevokedAt: { id: 'app.edgeStatus.devices.revokedAt', defaultMessage: 'revoked {at}', description: 'Edge status: when a device was revoked' },
  esRevoke: { id: 'app.edgeStatus.devices.revoke', defaultMessage: 'Revoke {name}', description: 'Edge status: revoke button; name is the device' },
  esConfirmRevoke: { id: 'app.edgeStatus.devices.confirmRevoke', defaultMessage: 'Yes, revoke {name}', description: 'Edge status: confirm revoking a device' },
  esCancel: { id: 'app.edgeStatus.devices.cancel', defaultMessage: 'Cancel', description: 'Edge status: cancel revoking' },
  esActionFailed: { id: 'app.edgeStatus.devices.failed', defaultMessage: 'That did not work: {reason}.', description: 'Edge status: an approval or revocation failed; reason is a code' },
  esPairingUnavailable: { id: 'app.edgeStatus.pairing.unavailable', defaultMessage: 'Pairing is unavailable: {reason}.', description: 'Edge status: the pairing payload could not be made; reason is a code' },
  esSystemVolume: { id: 'app.edgeStatus.systemVolume', defaultMessage: 'system disk', description: 'Edge status: the volume holding the operating system' },
  esSystemDisk: { id: 'app.edgeStatus.systemDisk', defaultMessage: 'System disk', description: 'Edge status: label for the volume holding the operating system' },
  esEncryption: { id: 'app.edgeStatus.encryption', defaultMessage: 'Disk encryption', description: 'Edge status: disk encryption section' },
  esTpm: { id: 'app.edgeStatus.tpm', defaultMessage: 'TPM 2.0', description: 'Edge status: whether the machine has a TPM 2.0' },
  esSecureBoot: { id: 'app.edgeStatus.secureBoot', defaultMessage: 'Secure Boot', description: 'Edge status: whether Secure Boot is on' },
  esYes: { id: 'app.edgeStatus.yes', defaultMessage: 'Yes', description: 'Edge status: a check that holds' },
  esNo: { id: 'app.edgeStatus.no', defaultMessage: 'No', description: 'Edge status: a check that does not hold' },
  esExemption: { id: 'app.edgeStatus.exemption', defaultMessage: 'Exemption', description: 'Edge status: the country-configuration grant exempting the facility' },
  esProtTpm: { id: 'app.edgeStatus.protection.tpm', defaultMessage: 'Encrypted, unlocked by the TPM', description: 'Disk protection state' },
  esProtSuspended: { id: 'app.edgeStatus.protection.suspended', defaultMessage: 'Encrypted, protection suspended', description: 'Disk protection state' },
  esProtNoTpm: { id: 'app.edgeStatus.protection.noTpm', defaultMessage: 'Encrypted, no TPM unlock', description: 'Disk protection state' },
  esProtNotSealed: { id: 'app.edgeStatus.protection.notSealed', defaultMessage: 'Encrypted, TPM unlock not tied to Secure Boot', description: 'Disk protection state' },
  esProtUnencrypted: { id: 'app.edgeStatus.protection.unencrypted', defaultMessage: 'Not encrypted', description: 'Disk protection state' },
  esProtUnknown: { id: 'app.edgeStatus.protection.unknown', defaultMessage: 'Could not be checked: {reason}', description: 'Disk protection state; reason is the probe error' },
  esNode: { id: 'app.edgeStatus.node', defaultMessage: 'Node', description: 'Edge status: node section' },
  esNodeId: { id: 'app.edgeStatus.nodeId', defaultMessage: 'Node ID', description: 'Edge status' },
  esVersion: { id: 'app.edgeStatus.version', defaultMessage: 'Version', description: 'Edge status' },
  esStarted: { id: 'app.edgeStatus.started', defaultMessage: 'Running since', description: 'Edge status' },
  esChecked: { id: 'app.edgeStatus.checked', defaultMessage: 'Checked', description: 'Edge status: when the report was taken' },
  esStorage: { id: 'app.edgeStatus.storage', defaultMessage: 'Storage', description: 'Edge status: storage section' },
  esStoreSize: { id: 'app.edgeStatus.storeSize', defaultMessage: 'Database size', description: 'Edge status' },
  esDataVolume: { id: 'app.edgeStatus.dataVolume', defaultMessage: 'data disk', description: 'Edge status: the volume holding the database' },
  esBackupVolume: { id: 'app.edgeStatus.backupVolume', defaultMessage: 'backup disk', description: 'Edge status: the volume holding backups' },
  esDataDisk: { id: 'app.edgeStatus.dataDisk', defaultMessage: 'Data disk', description: 'Edge status: label for the volume holding the database' },
  esBackupDisk: { id: 'app.edgeStatus.backupDisk', defaultMessage: 'Backup disk', description: 'Edge status: label for the volume holding backups' },
  esVolumeUse: { id: 'app.edgeStatus.volumeUse', defaultMessage: '{percent}% used, {free} free', description: 'Edge status: how full a volume is' },
  esBackups: { id: 'app.edgeStatus.backups', defaultMessage: 'Backups', description: 'Edge status: backups section' },
  esBackupsOff: { id: 'app.edgeStatus.backupsOff', defaultMessage: 'Not configured', description: 'Edge status: no backups' },
  esLastBackup: { id: 'app.edgeStatus.lastBackup', defaultMessage: 'Last good backup', description: 'Edge status' },
  esLastFailure: { id: 'app.edgeStatus.lastFailure', defaultMessage: 'Last failure', description: 'Edge status' },
  esLastTest: { id: 'app.edgeStatus.lastTest', defaultMessage: 'Last test restore', description: 'Edge status' },
  esTestPassed: { id: 'app.edgeStatus.testPassed', defaultMessage: 'Passed, {at}', description: 'Edge status: test restore passed at a time' },
  esTestFailed: { id: 'app.edgeStatus.testFailed', defaultMessage: 'Failed, {at}', description: 'Edge status: test restore failed at a time' },
  esPower: { id: 'app.edgeStatus.power', defaultMessage: 'Power', description: 'Edge status: power section' },
  esUps: { id: 'app.edgeStatus.ups', defaultMessage: 'UPS', description: 'Edge status' },
  esUpsOff: { id: 'app.edgeStatus.upsOff', defaultMessage: 'Not configured', description: 'Edge status: no UPS' },
  esPowerOnline: { id: 'app.edgeStatus.power.online', defaultMessage: 'On mains power', description: 'UPS state' },
  esPowerOnBattery: { id: 'app.edgeStatus.power.onBattery', defaultMessage: 'On battery', description: 'UPS state' },
  esPowerLowBattery: { id: 'app.edgeStatus.power.lowBattery', defaultMessage: 'Low battery', description: 'UPS state' },
  esPowerUnreachable: { id: 'app.edgeStatus.power.unreachable', defaultMessage: 'Not answering', description: 'UPS state' },
  esTelemetry: { id: 'app.edgeStatus.telemetry', defaultMessage: 'Monitoring', description: 'Edge status: telemetry section' },
  esBuffered: { id: 'app.edgeStatus.buffered', defaultMessage: 'Waiting to send to central', description: 'Edge status: telemetry buffered' },
  esDropped: { id: 'app.edgeStatus.dropped', defaultMessage: 'Dropped when the buffer was full', description: 'Edge status' },
  esLastShipped: { id: 'app.edgeStatus.lastShipped', defaultMessage: 'Last sent to central', description: 'Edge status' },
  esShippingOff: { id: 'app.edgeStatus.shippingOff', defaultMessage: 'Not configured: kept on this node', description: 'Edge status: no collector' },
  esShippingFailing: { id: 'app.edgeStatus.shippingFailing', defaultMessage: 'Central unreachable; last sent {at}', description: 'Edge status: shipping failing' },
  esTaskRestarts: { id: 'app.edgeStatus.taskRestarts', defaultMessage: 'Recent restarts of {task}', description: 'Edge status: a background task' },
  esNever: { id: 'app.edgeStatus.never', defaultMessage: 'Never', description: 'Edge status: no time yet' },
  esUnknown: { id: 'app.edgeStatus.unknown', defaultMessage: 'Unknown', description: 'Edge status: not measured' },
  esPending: { id: 'app.edgeStatus.pending', defaultMessage: 'Not reported yet: {indicators}.', description: 'Edge status: indicators that arrive with later work' },
})

/** The device enrollment page's messages' ICU arguments. */
type EnrollmentValues = {
  enTitle: {}
  enIntro: {}
  enStart: {}
  enCode: {}
  enShowCode: { expires: string }
  enWaiting: {}
  enEnrolled: { name: string; location: string }
  enRevoked: {}
  enStartOver: {}
  enExpired: {}
  enNotEdge: {}
  enFailed: { reason: string }
}

// The device enrollment page's strings (features/enrollment).
export const enrollmentMessages = defineMessages<EnrollmentValues>({
  enTitle: { id: 'app.enroll.title', defaultMessage: 'Enroll this device', description: 'Device enrollment page heading and home link' },
  enIntro: {
    id: 'app.enroll.intro',
    defaultMessage: 'This device needs a facility admin to approve it before it can use the facility edge.',
    description: 'Device enrollment: why the device must be enrolled',
  },
  enStart: { id: 'app.enroll.start', defaultMessage: 'Ask to enroll', description: 'Device enrollment: button that sends the request' },
  enCode: { id: 'app.enroll.code', defaultMessage: 'Pairing code', description: 'Device enrollment: label of the code the admin types' },
  enShowCode: {
    id: 'app.enroll.showCode',
    defaultMessage: 'Give this code to the facility admin, who approves the device on the edge. It expires at {expires}.',
    description: 'Device enrollment: what to do with the pairing code; expires is a time',
  },
  enWaiting: { id: 'app.enroll.waiting', defaultMessage: 'Waiting for approval…', description: 'Device enrollment: polling for approval' },
  enEnrolled: {
    id: 'app.enroll.enrolled',
    defaultMessage: 'This device is enrolled as {name}, at {location}.',
    description: 'Device enrollment: the device was approved',
  },
  enRevoked: {
    id: 'app.enroll.revoked',
    defaultMessage: 'This device was revoked. To use it again, start over and ask the admin to approve it.',
    description: 'Device enrollment: the device was revoked',
  },
  enStartOver: { id: 'app.enroll.startOver', defaultMessage: 'Start over', description: 'Device enrollment: forget the key and enroll again' },
  enExpired: {
    id: 'app.enroll.expired',
    defaultMessage: 'The last request was not approved in time. Ask again for a new code.',
    description: 'Device enrollment: the pending request expired',
  },
  enNotEdge: { id: 'app.enroll.notEdge', defaultMessage: 'Devices enroll with a facility edge, not with central.', description: 'Device enrollment: shown away from an edge' },
  enFailed: { id: 'app.enroll.failed', defaultMessage: 'The request failed: {reason}.', description: 'Device enrollment: a request failed; reason is a code' },
})

type AccessValues = {
  acTitle: {}
  acIntro: {}
  acLoading: {}
  acForbidden: {}
  acFailed: {}
  acUsername: {}
  acRole: {}
  acChooseRole: {}
  acDays: {}
  acDayCount: { days: number }
  acGrants: {}
  acNoGrants: {}
  acGrantLine: { username: string; role: string; until: string }
  acGrant: {}
  acGranted: { username: string; role: string; until: string }
  acNewStaff: {}
  acNewStaffIntro: {}
  acTemporaryPassword: {}
  acTemporaryPasswordHelp: {}
  acCreate: {}
  acCreated: { username: string; role: string; until: string }
  acRevocations: {}
  acRevokeIntro: {}
  acNoRevocations: {}
  acRevocationLine: { username: string; at: string }
  acRevoke: {}
  acConfirmRevoke: { username: string }
  acCancel: {}
  acRevoked: { username: string }
  acRefused: { reason: string }
  acNoSuchUser: {}
  acTaken: {}
}

// The offline administration page's strings (features/access).
export const accessMessages = defineMessages<AccessValues>({
  acTitle: { id: 'app.access.title', defaultMessage: 'Access at this facility', description: 'Offline administration page heading and home link' },
  acIntro: {
    id: 'app.access.intro',
    defaultMessage: 'While the facility is offline, grant a role for up to 7 days, create an account for new staff, or revoke access here. Central confirms or reverses each change when the facility reconnects.',
    description: 'Offline administration page: what it is for',
  },
  acLoading: { id: 'app.access.loading', defaultMessage: 'Loading…', description: 'Offline administration page: loading' },
  acForbidden: { id: 'app.access.forbidden', defaultMessage: 'Your role does not include administering access here.', description: 'Offline administration page: the user is not a facility admin' },
  acFailed: { id: 'app.access.failed', defaultMessage: 'Access administration could not be loaded. Please try again.', description: 'Offline administration page: loading failed' },
  acUsername: { id: 'app.access.username', defaultMessage: 'Username', description: 'Offline administration: whose access' },
  acRole: { id: 'app.access.role', defaultMessage: 'Role', description: 'Offline administration: the role to grant' },
  acChooseRole: { id: 'app.access.chooseRole', defaultMessage: 'Choose a role', description: 'Offline administration: no role chosen yet' },
  acDays: { id: 'app.access.days', defaultMessage: 'For how long', description: 'Offline administration: how long the grant lasts' },
  acDayCount: { id: 'app.access.dayCount', defaultMessage: '{days, plural, one {# day} other {# days}}', description: 'Offline administration: a duration option' },
  acGrants: { id: 'app.access.grants', defaultMessage: 'Roles granted here', description: 'Offline administration: the local grants section' },
  acNoGrants: { id: 'app.access.noGrants', defaultMessage: 'No roles are granted here.', description: 'Offline administration: no local grants' },
  acGrantLine: { id: 'app.access.grantLine', defaultMessage: '{username}: {role} until {until}', description: 'Offline administration: one local grant' },
  acGrant: { id: 'app.access.grant', defaultMessage: 'Grant the role', description: 'Offline administration: grant button' },
  acGranted: { id: 'app.access.granted', defaultMessage: '{username} is a {role} here until {until}.', description: 'Offline administration: the role was granted' },
  acNewStaff: { id: 'app.access.newStaff', defaultMessage: 'Account for new staff', description: 'Offline administration: the provisional account section' },
  acNewStaffIntro: {
    id: 'app.access.newStaffIntro',
    defaultMessage: 'For someone central has not registered yet. The account lasts as long as its role, and its user chooses their own password when they first sign in.',
    description: 'Offline administration: what a provisional account is',
  },
  acTemporaryPassword: { id: 'app.access.temporaryPassword', defaultMessage: 'Temporary password', description: 'Offline administration: the password the admin sets' },
  acTemporaryPasswordHelp: {
    id: 'app.access.temporaryPasswordHelp',
    defaultMessage: 'Give it to them in person. They must replace it at their first sign-in.',
    description: 'Offline administration: how to hand over the temporary password',
  },
  acCreate: { id: 'app.access.create', defaultMessage: 'Create the account', description: 'Offline administration: create a provisional account' },
  acCreated: {
    id: 'app.access.created',
    defaultMessage: 'The account {username} is a {role} here until {until}.',
    description: 'Offline administration: the provisional account was created',
  },
  acRevocations: { id: 'app.access.revocations', defaultMessage: 'Access revoked here', description: 'Offline administration: the revocations section' },
  acRevokeIntro: {
    id: 'app.access.revokeIntro',
    defaultMessage: 'Revoking takes effect at once: the user is signed out and cannot sign in here.',
    description: 'Offline administration: what revoking does',
  },
  acNoRevocations: { id: 'app.access.noRevocations', defaultMessage: 'No access is revoked here.', description: 'Offline administration: no revocations' },
  acRevocationLine: { id: 'app.access.revocationLine', defaultMessage: '{username}: revoked {at}', description: 'Offline administration: one revocation' },
  acRevoke: { id: 'app.access.revoke', defaultMessage: 'Revoke access', description: 'Offline administration: revoke button' },
  acConfirmRevoke: { id: 'app.access.confirmRevoke', defaultMessage: 'Revoke access for {username}', description: 'Offline administration: confirm the revocation' },
  acCancel: { id: 'app.access.cancel', defaultMessage: 'Cancel', description: 'Offline administration: do not revoke' },
  acRevoked: { id: 'app.access.revoked', defaultMessage: 'Access for {username} is revoked here.', description: 'Offline administration: access was revoked' },
  acRefused: { id: 'app.access.refused', defaultMessage: 'Not done: {reason}', description: 'Offline administration: the edge refused; reason is its explanation' },
  acNoSuchUser: { id: 'app.access.noSuchUser', defaultMessage: 'there is no account with that username here', description: 'Offline administration: unknown username' },
  acTaken: { id: 'app.access.taken', defaultMessage: 'that username is already taken', description: 'Offline administration: the username exists' },
})

type WorklistValues = {
  wlTitle: {}
  wlLoading: {}
  wlFailed: {}
  wlForbidden: {}
  wlNone: {}
  wlStartTitle: {}
  wlDefinition: {}
  wlContext: {}
  wlStart: {}
  wlStarted: {}
  wlStartFailed: {}
  wlWorklist: {}
  wlChoose: {}
  wlEmpty: {}
  wlColStep: {}
  wlColAbout: {}
  wlColPriority: {}
  wlColArrived: {}
  wlColStatus: {}
  wlReady: {}
  wlMine: {}
  wlTaken: {}
  wlClaim: {}
  wlRelease: {}
  wlComplete: { event: string }
  wlActionFailed: { reason: string }
  wlHint: {}
}

// The worklists' strings (features/worklists).
export const worklistMessages = defineMessages<WorklistValues>({
  wlTitle: { id: 'app.worklists.title', defaultMessage: 'Worklists', description: 'Worklists page heading and link' },
  wlLoading: { id: 'app.worklists.loading', defaultMessage: 'Loading…', description: 'Worklists: loading' },
  wlFailed: { id: 'app.worklists.failed', defaultMessage: 'The worklists could not be loaded.', description: 'Worklists: load failed' },
  wlForbidden: { id: 'app.worklists.forbidden', defaultMessage: 'You do not have access to worklists.', description: 'Worklists: no permission' },
  wlNone: { id: 'app.worklists.none', defaultMessage: 'No workflows run here yet.', description: 'Worklists: the node runs no definitions' },
  wlStartTitle: { id: 'app.worklists.startTitle', defaultMessage: 'Start a workflow', description: 'Worklists: start section heading' },
  wlDefinition: { id: 'app.worklists.definition', defaultMessage: 'Workflow', description: 'Worklists field: the definition to start' },
  wlContext: { id: 'app.worklists.context', defaultMessage: 'About (e.g. Encounter/123)', description: 'Worklists field: the context resource' },
  wlStart: { id: 'app.worklists.start', defaultMessage: 'Start', description: 'Worklists action: start an instance' },
  wlStarted: { id: 'app.worklists.started', defaultMessage: 'Started.', description: 'Worklists: an instance was started' },
  wlStartFailed: { id: 'app.worklists.startFailed', defaultMessage: 'The workflow was not started.', description: 'Worklists: start failed' },
  wlWorklist: { id: 'app.worklists.worklist', defaultMessage: 'Worklist', description: 'Worklists field: which worklist' },
  wlChoose: { id: 'app.worklists.choose', defaultMessage: 'Choose a worklist', description: 'Worklists: no worklist chosen' },
  wlEmpty: { id: 'app.worklists.empty', defaultMessage: 'Nothing is waiting on this worklist.', description: 'Worklists: empty worklist' },
  wlColStep: { id: 'app.worklists.colStep', defaultMessage: 'Step', description: 'Worklists column: the engine state' },
  wlColAbout: { id: 'app.worklists.colAbout', defaultMessage: 'About', description: 'Worklists column: the context' },
  wlColPriority: { id: 'app.worklists.colPriority', defaultMessage: 'Priority', description: 'Worklists column' },
  wlColArrived: { id: 'app.worklists.colArrived', defaultMessage: 'Arrived', description: 'Worklists column: when the task arrived' },
  wlColStatus: { id: 'app.worklists.colStatus', defaultMessage: 'Status', description: 'Worklists column' },
  wlReady: { id: 'app.worklists.ready', defaultMessage: 'Waiting', description: 'Worklists: task status ready' },
  wlMine: { id: 'app.worklists.mine', defaultMessage: 'You have it', description: 'Worklists: task status, claimed by the signed-in user' },
  wlTaken: { id: 'app.worklists.taken', defaultMessage: 'Taken by a colleague', description: 'Worklists: task status, claimed by someone else' },
  wlClaim: { id: 'app.worklists.claim', defaultMessage: 'Take this task', description: 'Worklists action: claim' },
  wlRelease: { id: 'app.worklists.release', defaultMessage: 'Put it back', description: 'Worklists action: release a claim' },
  wlComplete: { id: 'app.worklists.complete', defaultMessage: 'Done: {event}', description: 'Worklists action: complete the step with an event' },
  wlActionFailed: { id: 'app.worklists.actionFailed', defaultMessage: 'That did not work: {reason}', description: 'Worklists: a command was refused' },
  wlHint: { id: 'app.worklists.hint', defaultMessage: 'Most urgent first, then by arrival. Select a task to work on it.', description: 'Worklists: table guidance' },
})

type FormValues = {
  fmYes: {}
  fmNo: {}
  fmChoose: {}
  fmRequired: {}
  fmAdd: { item: string }
  fmRemove: { item: string }
  fmCalculated: {}
  fmNotNumber: {}
  fmMissing: { count: number }
  fmSubmit: {}
  fmSave: {}
  fmTitle: {}
  fmLoading: {}
  fmFailed: {}
  fmNone: {}
  fmChooseForm: {}
  fmOpen: {}
  fmDrafts: {}
  fmContinue: { saved: string }
  fmSaved: {}
  fmSubmitted: {}
  fmSaveFailed: {}
  fmConflict: {}
}

// The Questionnaire renderer's strings (features/forms).
export const formMessages = defineMessages<FormValues>({
  fmYes: { id: 'app.forms.yes', defaultMessage: 'Yes', description: 'Forms: a Boolean answer' },
  fmNo: { id: 'app.forms.no', defaultMessage: 'No', description: 'Forms: a Boolean answer' },
  fmChoose: { id: 'app.forms.choose', defaultMessage: 'Choose', description: 'Forms: no option chosen' },
  fmRequired: { id: 'app.forms.required', defaultMessage: 'Required', description: 'Forms: a required question' },
  fmAdd: { id: 'app.forms.add', defaultMessage: 'Add another: {item}', description: 'Forms: add a repeat of a group' },
  fmRemove: { id: 'app.forms.remove', defaultMessage: 'Remove this: {item}', description: 'Forms: remove a repeat of a group' },
  fmCalculated: { id: 'app.forms.calculated', defaultMessage: 'Calculated', description: 'Forms: a value the form calculates' },
  fmNotNumber: { id: 'app.forms.notNumber', defaultMessage: 'Enter a number', description: 'Forms: a number answer that is not a number' },
  fmMissing: {
    id: 'app.forms.missing',
    defaultMessage: '{count, plural, one {# required question is} other {# required questions are}} not answered.',
    description: 'Forms: required questions unanswered on submit',
  },
  fmSubmit: { id: 'app.forms.submit', defaultMessage: 'Submit', description: 'Forms: submit the response' },
  fmSave: { id: 'app.forms.save', defaultMessage: 'Save draft', description: 'Forms: save the response as a draft' },
  fmTitle: { id: 'app.forms.title', defaultMessage: 'Forms', description: 'Forms page heading and link' },
  fmLoading: { id: 'app.forms.loading', defaultMessage: 'Loading…', description: 'Forms: loading' },
  fmFailed: { id: 'app.forms.failed', defaultMessage: 'The forms could not be loaded.', description: 'Forms: load failed' },
  fmNone: { id: 'app.forms.none', defaultMessage: 'No forms are available here yet.', description: 'Forms: no Questionnaires' },
  fmChooseForm: { id: 'app.forms.chooseForm', defaultMessage: 'Form', description: 'Forms field: which Questionnaire' },
  fmOpen: { id: 'app.forms.open', defaultMessage: 'Start a new response', description: 'Forms action: open a new response' },
  fmDrafts: { id: 'app.forms.drafts', defaultMessage: 'Drafts', description: 'Forms: saved drafts heading' },
  fmContinue: { id: 'app.forms.continue', defaultMessage: 'Continue the draft saved {saved}', description: 'Forms action: continue a saved draft' },
  fmSaved: { id: 'app.forms.saved', defaultMessage: 'Draft saved.', description: 'Forms: a draft was saved' },
  fmSubmitted: { id: 'app.forms.submitted', defaultMessage: 'Submitted.', description: 'Forms: the response was submitted' },
  fmSaveFailed: { id: 'app.forms.saveFailed', defaultMessage: 'The response was not saved. The server checked it and refused it.', description: 'Forms: save or submit refused' },
  fmConflict: { id: 'app.forms.conflict', defaultMessage: 'Someone saved this response elsewhere. Open the latest draft to continue.', description: 'Forms: the draft changed on another device' },
})

type ReceptionValues = {
  rcTitle: {}
  rcFamily: {}
  rcGiven: {}
  rcFind: {}
  rcNoneFound: {}
  rcChoose: { name: string }
  rcChosen: { name: string }
  rcServicePoint: {}
  rcEmergency: {}
  rcCheckIn: {}
  rcCheckedIn: { name: string; servicePoint: string }
  rcFailed: { reason: string }
  rcQueuesTitle: {}
  rcQueueEmpty: {}
  rcWaiting: {}
  rcCalled: {}
  rcInService: {}
  rcCall: { name: string }
  rcServe: { name: string }
  rcFinish: { name: string }
  rcLeft: { name: string }
  rcEmergencyTag: {}
  rcArrived: { time: string }
}

// Reception and queues' strings (features/reception).
export const receptionMessages = defineMessages<ReceptionValues>({
  rcTitle: { id: 'app.reception.title', defaultMessage: 'Reception', description: 'Reception page heading and link' },
  rcFamily: { id: 'app.reception.family', defaultMessage: 'Family name', description: 'Reception field' },
  rcGiven: { id: 'app.reception.given', defaultMessage: 'Given names', description: 'Reception field' },
  rcFind: { id: 'app.reception.find', defaultMessage: 'Find the patient', description: 'Reception action: search patients' },
  rcNoneFound: { id: 'app.reception.noneFound', defaultMessage: 'No patient matches. Register them first.', description: 'Reception: no search results' },
  rcChoose: { id: 'app.reception.choose', defaultMessage: 'Choose {name}', description: 'Reception action: pick a patient from the results' },
  rcChosen: { id: 'app.reception.chosen', defaultMessage: 'Patient: {name}', description: 'Reception: the chosen patient' },
  rcServicePoint: { id: 'app.reception.servicePoint', defaultMessage: 'Service point', description: 'Reception field: where the patient goes' },
  rcEmergency: { id: 'app.reception.emergency', defaultMessage: 'Emergency', description: 'Reception field: the patient is an emergency' },
  rcCheckIn: { id: 'app.reception.checkIn', defaultMessage: 'Check in', description: 'Reception action' },
  rcCheckedIn: { id: 'app.reception.checkedIn', defaultMessage: '{name} is checked in at {servicePoint}.', description: 'Reception: checked in' },
  rcFailed: { id: 'app.reception.failed', defaultMessage: 'That did not work: {reason}', description: 'Reception: a request was refused' },
  rcQueuesTitle: { id: 'app.reception.queuesTitle', defaultMessage: 'Queues', description: 'Queues page heading and link' },
  rcQueueEmpty: { id: 'app.reception.queueEmpty', defaultMessage: 'Nobody is waiting here.', description: 'Queues: empty queue' },
  rcWaiting: { id: 'app.reception.waiting', defaultMessage: 'Waiting', description: 'Queues: entry status' },
  rcCalled: { id: 'app.reception.called', defaultMessage: 'Called', description: 'Queues: entry status' },
  rcInService: { id: 'app.reception.inService', defaultMessage: 'Being seen', description: 'Queues: entry status' },
  rcCall: { id: 'app.reception.call', defaultMessage: 'Call {name}', description: 'Queues action' },
  rcServe: { id: 'app.reception.serve', defaultMessage: 'Start seeing {name}', description: 'Queues action' },
  rcFinish: { id: 'app.reception.finish', defaultMessage: 'Done with {name}', description: 'Queues action' },
  rcLeft: { id: 'app.reception.left', defaultMessage: '{name} left', description: 'Queues action: left without being seen' },
  rcEmergencyTag: { id: 'app.reception.emergencyTag', defaultMessage: 'Emergency', description: 'Queues: an emergency entry' },
  rcArrived: { id: 'app.reception.arrived', defaultMessage: 'Arrived {time}', description: 'Queues: when the patient arrived' },
})

type ConsultationValues = {
  csTitle: {}
  csWaiting: {}
  csNoneWaiting: {}
  csSee: { name: string }
  csWith: { name: string }
  csSummary: {}
  csAllergies: {}
  csConditions: {}
  csMedications: {}
  csVitals: {}
  csVisits: {}
  csNotShown: {}
  csNoneRecorded: {}
  csNotes: {}
  csNoteKind: {}
  csKind: { kind: string }
  csSaveDraft: {}
  csSign: {}
  csSignDraft: { kind: string }
  csAmend: { kind: string }
  csSignAmendment: {}
  csDraftSaved: {}
  csNoteSigned: {}
  csAmendmentSigned: {}
  csNoNotes: {}
  csStatus: { status: string }
  csSignedBy: { name: string; date: string }
  csAmends: {}
  csAmending: { kind: string }
  csDiagnosis: {}
  csProvisional: {}
  csAddDiagnosis: {}
  csDiagnosisAdded: { name: string }
  csTest: {}
  csOrderTest: {}
  csTestOrdered: { name: string }
  csMedicine: {}
  csDosage: {}
  csPrescribe: {}
  csPrescribed: { name: string }
  csWarnings: {}
  cpTitle: {}
  cpName: {}
  cpStart: {}
  cpStarted: {}
  cpNone: {}
  cpStatus: { status: string }
  cpAddTo: { title: string }
  cpAdd: {}
  cpAdded: {}
  cpAddresses: { what: string }
  csPrescribeAnyway: {}
  csSendTo: {}
  csSend: {}
  csComplete: {}
  csSent: { servicePoint: string }
  csCompleted: {}
  csBack: {}
  csFailed: { reason: string }
}

// The consultation's strings (features/consultation).
export const consultationMessages = defineMessages<ConsultationValues>({
  csTitle: { id: 'app.consultation.title', defaultMessage: 'Consultation', description: 'Consultation page heading and link' },
  csWaiting: { id: 'app.consultation.waiting', defaultMessage: 'Waiting for consultation', description: 'Consultation: the queue heading' },
  csNoneWaiting: { id: 'app.consultation.noneWaiting', defaultMessage: 'Nobody is waiting for consultation.', description: 'Consultation: empty queue' },
  csSee: { id: 'app.consultation.see', defaultMessage: 'See {name}', description: 'Consultation action: open the patient' },
  csWith: { id: 'app.consultation.with', defaultMessage: 'Consultation: {name}', description: 'Consultation: the workspace heading' },
  csSummary: { id: 'app.consultation.summary', defaultMessage: 'Summary', description: 'Consultation: the patient summary heading' },
  csAllergies: { id: 'app.consultation.allergies', defaultMessage: 'Allergies', description: 'Summary section' },
  csConditions: { id: 'app.consultation.conditions', defaultMessage: 'Conditions', description: 'Summary section' },
  csMedications: { id: 'app.consultation.medications', defaultMessage: 'Medications', description: 'Summary section' },
  csVitals: { id: 'app.consultation.vitals', defaultMessage: 'Vital signs', description: 'Summary section' },
  csVisits: { id: 'app.consultation.visits', defaultMessage: 'Recent visits', description: 'Summary section' },
  csNotShown: { id: 'app.consultation.notShown', defaultMessage: 'Not available to you.', description: 'Summary: a section the user may not see' },
  csNoneRecorded: { id: 'app.consultation.noneRecorded', defaultMessage: 'None recorded.', description: 'Summary: an empty section' },
  csNotes: { id: 'app.consultation.notes', defaultMessage: 'Notes', description: 'Consultation: the notes heading' },
  csNoteKind: { id: 'app.consultation.noteKind', defaultMessage: 'Kind of note', description: 'Consultation field' },
  csKind: {
    id: 'app.consultation.kind',
    defaultMessage:
      '{kind, select, progress {Progress note (SOAP)} consultation {Consultation note} admission {Admission note} procedure {Procedure note} discharge {Discharge summary} letter {Letter} other {Note}}',
    description: 'A kind of clinical note',
  },
  csSaveDraft: { id: 'app.consultation.saveDraft', defaultMessage: 'Save draft', description: 'Consultation action: save the note unsigned' },
  csSign: { id: 'app.consultation.sign', defaultMessage: 'Sign', description: 'Consultation action: sign the note being written' },
  csSignDraft: { id: 'app.consultation.signDraft', defaultMessage: 'Sign the draft {kind}', description: 'Consultation action: sign a saved draft' },
  csAmend: { id: 'app.consultation.amend', defaultMessage: 'Amend the {kind}', description: 'Consultation action: amend a signed note' },
  csSignAmendment: { id: 'app.consultation.signAmendment', defaultMessage: 'Sign amendment', description: 'Consultation action' },
  csDraftSaved: { id: 'app.consultation.draftSaved', defaultMessage: 'Draft saved.', description: 'Consultation: draft saved' },
  csNoteSigned: { id: 'app.consultation.noteSigned', defaultMessage: 'Note signed.', description: 'Consultation: note signed' },
  csAmendmentSigned: { id: 'app.consultation.amendmentSigned', defaultMessage: 'Amendment signed.', description: 'Consultation: amendment signed' },
  csNoNotes: { id: 'app.consultation.noNotes', defaultMessage: 'No notes here yet.', description: 'Consultation: no notes' },
  csStatus: {
    id: 'app.consultation.status',
    defaultMessage:
      '{status, select, preliminary {Draft} final {Signed} amended {Amendment, signed} appended {Addendum, signed} corrected {Correction, signed} cancelled {Withdrawn} enteredinerror {Entered in error} other {Unknown}}',
    description: 'A note status (its FHIR code without hyphens)',
  },
  csSignedBy: { id: 'app.consultation.signedBy', defaultMessage: 'Signed by {name}, {date}', description: 'Consultation: who signed a note' },
  csAmends: { id: 'app.consultation.amends', defaultMessage: 'Amends an earlier note', description: 'Consultation: an amendment' },
  csAmending: { id: 'app.consultation.amending', defaultMessage: 'Amending the {kind}', description: 'Consultation: the amendment editor heading' },
  csDiagnosis: { id: 'app.consultation.diagnosis', defaultMessage: 'Diagnosis', description: 'Consultation field' },
  csProvisional: { id: 'app.consultation.provisional', defaultMessage: 'Provisional', description: 'Consultation field: the diagnosis is not confirmed' },
  csAddDiagnosis: { id: 'app.consultation.addDiagnosis', defaultMessage: 'Add diagnosis', description: 'Consultation action' },
  csDiagnosisAdded: { id: 'app.consultation.diagnosisAdded', defaultMessage: 'Diagnosis added: {name}.', description: 'Consultation: diagnosis recorded' },
  csTest: { id: 'app.consultation.test', defaultMessage: 'Test or procedure', description: 'Consultation field' },
  csOrderTest: { id: 'app.consultation.orderTest', defaultMessage: 'Order', description: 'Consultation action: order a test or procedure' },
  csTestOrdered: { id: 'app.consultation.testOrdered', defaultMessage: 'Ordered: {name}.', description: 'Consultation: test ordered' },
  csMedicine: { id: 'app.consultation.medicine', defaultMessage: 'Medicine', description: 'Consultation field' },
  csDosage: { id: 'app.consultation.dosage', defaultMessage: 'Dosage', description: 'Consultation field' },
  csPrescribe: { id: 'app.consultation.prescribe', defaultMessage: 'Prescribe', description: 'Consultation action' },
  csPrescribed: { id: 'app.consultation.prescribed', defaultMessage: 'Prescribed: {name}.', description: 'Consultation: medicine prescribed' },
  cpTitle: { id: 'app.consultation.carePlans', defaultMessage: 'Care plans', description: 'Consultation: the care plans heading' },
  cpName: { id: 'app.consultation.carePlanName', defaultMessage: 'Care plan title', description: 'Consultation field' },
  cpStart: { id: 'app.consultation.startCarePlan', defaultMessage: 'Start care plan', description: 'Consultation action' },
  cpStarted: { id: 'app.consultation.carePlanStarted', defaultMessage: 'Care plan started.', description: 'Consultation: a care plan started' },
  cpNone: { id: 'app.consultation.noCarePlans', defaultMessage: 'No care plans yet.', description: 'Consultation: no care plans' },
  cpStatus: {
    id: 'app.consultation.carePlanStatus',
    defaultMessage: '{status, select, draft {Draft} active {Active} onhold {On hold} revoked {Revoked} completed {Completed} enteredinerror {Entered in error} other {Unknown}}',
    description: 'A care plan status (its code without hyphens)',
  },
  cpAddTo: { id: 'app.consultation.addTo', defaultMessage: 'Add to {title}', description: 'Consultation field: what to add to a care plan' },
  cpAdd: { id: 'app.consultation.addToPlan', defaultMessage: 'Add to plan', description: 'Consultation action' },
  cpAdded: { id: 'app.consultation.addedToPlan', defaultMessage: 'Added to the care plan.', description: 'Consultation: added to a care plan' },
  cpAddresses: { id: 'app.consultation.addresses', defaultMessage: 'For: {what}', description: 'Consultation: what a care plan addresses' },
  csWarnings: { id: 'app.consultation.warnings', defaultMessage: 'Check before prescribing:', description: 'Consultation: clinical safety warnings heading' },
  csPrescribeAnyway: { id: 'app.consultation.prescribeAnyway', defaultMessage: 'I have checked: prescribe anyway', description: 'Consultation action: acknowledge the safety warnings and prescribe' },
  csSendTo: { id: 'app.consultation.sendTo', defaultMessage: 'Send the patient to', description: 'Consultation field: the next service point' },
  csSend: { id: 'app.consultation.send', defaultMessage: 'Send', description: 'Consultation action: move the patient' },
  csComplete: { id: 'app.consultation.complete', defaultMessage: 'Complete the visit', description: 'Consultation action' },
  csSent: { id: 'app.consultation.sent', defaultMessage: 'Sent to {servicePoint}.', description: 'Consultation: patient moved' },
  csCompleted: { id: 'app.consultation.completed', defaultMessage: 'The visit is complete.', description: 'Consultation: visit completed' },
  csBack: { id: 'app.consultation.back', defaultMessage: 'Back to the queue', description: 'Consultation action' },
  csFailed: { id: 'app.consultation.failed', defaultMessage: 'That did not work: {reason}', description: 'Consultation: a command was refused' },
})

type PharmacyValues = {
  phTitle: {}
  phWorklist: {}
  phEmpty: {}
  phFailed: {}
  phPrescription: { medicine: string; dosage: string }
  phDays: { days: number }
  phBy: { name: string; time: string }
  phDispense: { medicine: string; name: string }
  phStore: {}
  phProduct: {}
  phQuantity: {}
  phPrepare: {}
  phPrepared: {}
  phReserved: { batch: string; expiry: string; quantity: number }
  phHandOver: {}
  phHandedOver: {}
  phCancel: {}
  phCancelled: {}
  phDispenseFailed: { reason: string }
}

// The pharmacy's strings (features/pharmacy).
export const pharmacyMessages = defineMessages<PharmacyValues>({
  phTitle: { id: 'app.pharmacy.title', defaultMessage: 'Pharmacy', description: 'Pharmacy page heading and link' },
  phWorklist: { id: 'app.pharmacy.worklist', defaultMessage: 'Prescriptions to dispense', description: 'Pharmacy: the worklist heading' },
  phEmpty: { id: 'app.pharmacy.empty', defaultMessage: 'No prescriptions are waiting.', description: 'Pharmacy: empty worklist' },
  phFailed: { id: 'app.pharmacy.failed', defaultMessage: 'The worklist could not be loaded.', description: 'Pharmacy: the worklist failed' },
  phPrescription: { id: 'app.pharmacy.prescription', defaultMessage: '{medicine}: {dosage}', description: 'Pharmacy: a prescription' },
  phDays: { id: 'app.pharmacy.days', defaultMessage: '{days, plural, one {# day} other {# days}}', description: 'Pharmacy: the course length' },
  phBy: { id: 'app.pharmacy.by', defaultMessage: 'Prescribed by {name}, {time}', description: 'Pharmacy: who prescribed and when' },
  phDispense: { id: 'app.pharmacy.dispense', defaultMessage: 'Dispense {medicine} for {name}', description: 'Pharmacy: label of a prescription’s dispense' },
  phStore: { id: 'app.pharmacy.store', defaultMessage: 'Store', description: 'Pharmacy: the store dispensed from' },
  phProduct: { id: 'app.pharmacy.product', defaultMessage: 'Stock product', description: 'Pharmacy: the stock product given' },
  phQuantity: { id: 'app.pharmacy.quantity', defaultMessage: 'Quantity (base units)', description: 'Pharmacy: how much to dispense' },
  phPrepare: { id: 'app.pharmacy.prepare', defaultMessage: 'Prepare', description: 'Pharmacy action: reserve stock for a dispense' },
  phPrepared: {
    id: 'app.pharmacy.prepared',
    defaultMessage: 'Prepared. The stock is reserved; nothing has moved yet.',
    description: 'Pharmacy: a dispense was prepared',
  },
  phReserved: { id: 'app.pharmacy.reserved', defaultMessage: 'Batch {batch}, expires {expiry}: {quantity, number}', description: 'Pharmacy: a reserved batch' },
  phHandOver: { id: 'app.pharmacy.handOver', defaultMessage: 'Hand over', description: 'Pharmacy action: complete a dispense' },
  phHandedOver: { id: 'app.pharmacy.handedOver', defaultMessage: 'Handed over. The stock is dispensed.', description: 'Pharmacy: a dispense was completed' },
  phCancel: { id: 'app.pharmacy.cancel', defaultMessage: 'Cancel the dispense', description: 'Pharmacy action: cancel a prepared dispense' },
  phCancelled: { id: 'app.pharmacy.cancelled', defaultMessage: 'Cancelled. The reserved stock is free again.', description: 'Pharmacy: a dispense was cancelled' },
  phDispenseFailed: { id: 'app.pharmacy.dispenseFailed', defaultMessage: 'That did not work. {reason}', description: 'Pharmacy: a dispense step failed' },
})

type LabValues = {
  lbTitle: {}
  lbWorklist: {}
  lbList: { list: string }
  lbNone: {}
  lbOpen: { test: string; name: string }
  lbSpecimen: {}
  lbCollect: {}
  lbCollected: {}
  lbValue: {}
  lbUnit: {}
  lbReferenceLow: {}
  lbReferenceHigh: {}
  lbCriticalLow: {}
  lbCriticalHigh: {}
  lbSaveResults: {}
  lbResulted: {}
  lbRejectReason: {}
  lbReject: {}
  lbRejected: {}
  lbVerify: {}
  lbVerified: {}
  lbFlag: { flag: string }
  lbResult: { name: string; value: string; flag: string }
  lbFailed: { reason: string }
  rsTitle: {}
  rsNone: {}
  rsUrgent: {}
  rsReview: { test: string }
  rsReviewed: {}
  rsFor: { test: string; name: string }
}

// The laboratory's and the clinician's results' strings (features/laboratory).
export const labMessages = defineMessages<LabValues>({
  lbTitle: { id: 'app.lab.title', defaultMessage: 'Laboratory', description: 'Laboratory page heading and link' },
  lbWorklist: { id: 'app.lab.worklist', defaultMessage: 'Worklist', description: 'Laboratory field: which worklist' },
  lbList: {
    id: 'app.lab.list',
    defaultMessage: '{list, select, labcollect {Specimens to collect} labresult {Results to enter} labverify {Results to verify} other {Worklist}}',
    description: 'A laboratory worklist (its key without hyphens)',
  },
  lbNone: { id: 'app.lab.none', defaultMessage: 'Nothing is waiting here.', description: 'Laboratory: an empty worklist' },
  lbOpen: { id: 'app.lab.open', defaultMessage: 'Open {test} for {name}', description: 'Laboratory action: open a test' },
  lbSpecimen: { id: 'app.lab.specimen', defaultMessage: 'Specimen', description: 'Laboratory field: what was collected' },
  lbCollect: { id: 'app.lab.collect', defaultMessage: 'Record collection', description: 'Laboratory action' },
  lbCollected: { id: 'app.lab.collected', defaultMessage: 'Collection recorded.', description: 'Laboratory: specimen collected' },
  lbValue: { id: 'app.lab.value', defaultMessage: 'Value', description: 'Laboratory field: the result' },
  lbUnit: { id: 'app.lab.unit', defaultMessage: 'Unit', description: 'Laboratory field' },
  lbReferenceLow: { id: 'app.lab.referenceLow', defaultMessage: 'Reference range, low', description: 'Laboratory field' },
  lbReferenceHigh: { id: 'app.lab.referenceHigh', defaultMessage: 'Reference range, high', description: 'Laboratory field' },
  lbCriticalLow: { id: 'app.lab.criticalLow', defaultMessage: 'Critical below', description: 'Laboratory field' },
  lbCriticalHigh: { id: 'app.lab.criticalHigh', defaultMessage: 'Critical above', description: 'Laboratory field' },
  lbSaveResults: { id: 'app.lab.saveResults', defaultMessage: 'Save results', description: 'Laboratory action' },
  lbResulted: { id: 'app.lab.resulted', defaultMessage: 'Results saved; they wait for verification.', description: 'Laboratory: results entered' },
  lbRejectReason: { id: 'app.lab.rejectReason', defaultMessage: 'Why the specimen is rejected', description: 'Laboratory field' },
  lbReject: { id: 'app.lab.reject', defaultMessage: 'Reject specimen', description: 'Laboratory action' },
  lbRejected: { id: 'app.lab.rejected', defaultMessage: 'Specimen rejected.', description: 'Laboratory: specimen rejected' },
  lbVerify: { id: 'app.lab.verify', defaultMessage: 'Verify results', description: 'Laboratory action' },
  lbVerified: { id: 'app.lab.verified', defaultMessage: 'Verified and released to the clinician.', description: 'Laboratory: results verified' },
  lbFlag: {
    id: 'app.lab.flag',
    defaultMessage:
      '{flag, select, normal {Normal} low {Low} high {High} criticallow {Critically low} criticalhigh {Critically high} abnormal {Abnormal} critical {Critical} other {Unknown}}',
    description: 'A result flag (its code without hyphens)',
  },
  lbResult: { id: 'app.lab.result', defaultMessage: '{name}: {value} ({flag})', description: 'A laboratory result' },
  lbFailed: { id: 'app.lab.failed', defaultMessage: 'That did not work: {reason}', description: 'Laboratory: a step was refused' },
  rsTitle: { id: 'app.results.title', defaultMessage: 'Results to review', description: 'Results page heading and link' },
  rsNone: { id: 'app.results.none', defaultMessage: 'No results to review.', description: 'Results: none waiting' },
  rsUrgent: { id: 'app.results.urgent', defaultMessage: 'Urgent: critical result', description: 'Results: a critical result' },
  rsReview: { id: 'app.results.review', defaultMessage: 'Mark {test} reviewed', description: 'Results action' },
  rsReviewed: { id: 'app.results.reviewed', defaultMessage: 'Marked reviewed.', description: 'Results: reviewed' },
  rsFor: { id: 'app.results.for', defaultMessage: '{test} for {name}', description: 'Results: a test and its patient' },
})

type InboxValues = {
  ibTitle: {}
  ibEmpty: {}
  ibFailed: {}
  ibKind: { kind: string }
  ibUrgent: {}
  ibFor: { title: string; name: string }
  ibOpen: { title: string }
  ibNotFound: {}
  ibWhat: {}
  ibStatus: {}
  ibWhen: {}
  ibRaw: {}
}

// The clinical inbox's and the record view's strings (features/inbox, features/records).
export const inboxMessages = defineMessages<InboxValues>({
  ibTitle: { id: 'app.inbox.title', defaultMessage: 'Inbox', description: 'Inbox page heading and link' },
  ibEmpty: { id: 'app.inbox.empty', defaultMessage: 'Nothing is waiting for you.', description: 'Inbox: empty' },
  ibFailed: { id: 'app.inbox.failed', defaultMessage: 'The inbox could not be loaded.', description: 'Inbox: failed' },
  ibKind: {
    id: 'app.inbox.kind',
    defaultMessage: '{kind, select, result {Result to review} documentation {Note to finish} request {Patient request} other {Item}}',
    description: 'The kind of an inbox item',
  },
  ibUrgent: { id: 'app.inbox.urgent', defaultMessage: 'Urgent', description: 'Inbox: an urgent item' },
  ibFor: { id: 'app.inbox.for', defaultMessage: '{title} for {name}', description: 'Inbox: an item and its patient' },
  ibOpen: { id: 'app.inbox.open', defaultMessage: 'Open {title}', description: 'Inbox action: open the authoritative resource' },
  ibNotFound: { id: 'app.inbox.notFound', defaultMessage: 'This record is not available to you.', description: 'Record: not found or not permitted' },
  ibWhat: { id: 'app.inbox.what', defaultMessage: 'What', description: 'Record field label' },
  ibStatus: { id: 'app.inbox.status', defaultMessage: 'Status', description: 'Record field label' },
  ibWhen: { id: 'app.inbox.when', defaultMessage: 'When', description: 'Record field label' },
  ibRaw: { id: 'app.inbox.raw', defaultMessage: 'The full record', description: 'Record: the raw resource' },
})


type RequestValues = {
  rqTitle: {}
  rqNone: {}
  rqLoadFailed: {}
  rqRefresh: {}
  rqKind: { kind: string }
  rqState: { state: string }
  rqReasonGiven: { reason: string }
  rqWithdraw: { title: string }
  rqAsk: {}
  rqWhatKind: {}
  rqPrescription: {}
  rqNoPrescriptions: {}
  rqWhat: {}
  rqFor: {}
  rqFacility: {}
  rqFacilityHint: {}
  rqPreferred: {}
  rqNote: {}
  rqSend: {}
  rqSent: {}
  rqFailed: { reason: string }
  rqStaffTitle: {}
  rqStart: {}
  rqEnd: {}
  rqAccept: {}
  rqWarnings: {}
  rqAcceptAnyway: {}
  rqDeclineReason: {}
  rqDecline: {}
  rqAccepted: {}
  rqDeclined: {}
}

// Patients' requests (#114): the portal's, and staff's accept and decline (features/requests).
export const requestMessages = defineMessages<RequestValues>({
  rqTitle: { id: 'app.requests.title', defaultMessage: 'Your requests', description: 'Portal: heading of the patient’s requests' },
  rqNone: { id: 'app.requests.none', defaultMessage: 'You have not asked for anything yet.', description: 'Portal: no requests' },
  rqLoadFailed: { id: 'app.requests.loadFailed', defaultMessage: 'Your requests could not be loaded.', description: 'Portal: requests failed to load' },
  rqRefresh: { id: 'app.requests.refresh', defaultMessage: 'Check for updates', description: 'Portal action: reload the patient’s requests' },
  rqKind: {
    id: 'app.requests.kind',
    defaultMessage: '{kind, select, refill {Prescription refill} care {Test or other care} appointment {Appointment} other {Request}}',
    description: 'The kind of a patient request',
  },
  rqState: {
    id: 'app.requests.state',
    defaultMessage: '{state, select, pending {Waiting for staff} accepted {Accepted} other {Closed}}',
    description: 'Where a patient request is',
  },
  rqReasonGiven: { id: 'app.requests.reasonGiven', defaultMessage: 'Reason: {reason}', description: 'Why a request was declined or withdrawn' },
  rqWithdraw: { id: 'app.requests.withdraw', defaultMessage: 'Withdraw {title}', description: 'Portal action: withdraw a pending request' },
  rqAsk: { id: 'app.requests.ask', defaultMessage: 'Ask for something', description: 'Portal: heading of the request form' },
  rqWhatKind: { id: 'app.requests.whatKind', defaultMessage: 'What do you need?', description: 'Portal: the kind of request' },
  rqPrescription: { id: 'app.requests.prescription', defaultMessage: 'Prescription to refill', description: 'Portal: the prescription to refill' },
  rqNoPrescriptions: {
    id: 'app.requests.noPrescriptions',
    defaultMessage: 'There is no prescription you can ask a refill of.',
    description: 'Portal: no refillable prescriptions',
  },
  rqWhat: { id: 'app.requests.what', defaultMessage: 'What you need', description: 'Portal: what test or care is asked for' },
  rqFor: { id: 'app.requests.for', defaultMessage: 'What the appointment is for', description: 'Portal: an appointment’s reason' },
  rqFacility: { id: 'app.requests.facility', defaultMessage: 'Facility code', description: 'Portal: the facility asked' },
  rqFacilityHint: {
    id: 'app.requests.facilityHint',
    defaultMessage: 'The code on your clinic card, such as KAM013.',
    description: 'Portal: hint for the facility code',
  },
  rqPreferred: { id: 'app.requests.preferred', defaultMessage: 'Preferred day (optional)', description: 'Portal: an appointment’s preferred day' },
  rqNote: { id: 'app.requests.note', defaultMessage: 'Anything else (optional)', description: 'Portal: a note with the request' },
  rqSend: { id: 'app.requests.send', defaultMessage: 'Send request', description: 'Portal action: send a request' },
  rqSent: { id: 'app.requests.sent', defaultMessage: 'Your request was sent. Staff will look at it.', description: 'Portal: a request was sent' },
  rqFailed: { id: 'app.requests.failed', defaultMessage: 'That did not work. {reason}', description: 'A request action failed, with the server’s reason' },
  rqStaffTitle: { id: 'app.requests.staffTitle', defaultMessage: 'Patient request', description: 'Record: heading of the accept and decline controls' },
  rqStart: { id: 'app.requests.start', defaultMessage: 'Appointment starts', description: 'Record: a booking’s start' },
  rqEnd: { id: 'app.requests.end', defaultMessage: 'Appointment ends', description: 'Record: a booking’s end' },
  rqAccept: { id: 'app.requests.accept', defaultMessage: 'Accept', description: 'Record action: accept a patient request' },
  rqWarnings: { id: 'app.requests.warnings', defaultMessage: 'Safety warnings to acknowledge:', description: 'Record: the safety warnings of a refill' },
  rqAcceptAnyway: { id: 'app.requests.acceptAnyway', defaultMessage: 'Accept despite the warnings', description: 'Record action: accept, acknowledging warnings' },
  rqDeclineReason: { id: 'app.requests.declineReason', defaultMessage: 'Reason for declining', description: 'Record: why a request is declined' },
  rqDecline: { id: 'app.requests.decline', defaultMessage: 'Decline', description: 'Record action: decline a patient request' },
  rqAccepted: { id: 'app.requests.accepted', defaultMessage: 'Request accepted.', description: 'Record: a request was accepted' },
  rqDeclined: { id: 'app.requests.declined', defaultMessage: 'Request declined.', description: 'Record: a request was declined' },
})

type ProductValues = {
  pdTitle: {}
  pdIntro: {}
  pdNone: {}
  pdFailed: {}
  pdListed: { name: string; code: string; unit: string }
  pdSynonyms: { names: string }
  pdAdd: {}
  pdCode: {}
  pdCodeHint: {}
  pdName: {}
  pdBaseUnit: {}
  pdPackSize: {}
  pdSynonymsField: {}
  pdSynonymsHint: {}
  pdSave: {}
  pdSaved: { name: string }
  pdSaveFailed: { reason: string }
  pdEquipment: {}
  pdEquipmentHint: {}
  pdIsEquipment: {}
  pdUsefulLife: {}
  pdUsefulLifeHint: {}
}

// The product master (#128 follow-up): stock products and the medicines they fill (features/stock).
export const productMessages = defineMessages<ProductValues>({
  pdTitle: { id: 'app.products.title', defaultMessage: 'Products', description: 'Products page heading and link' },
  pdIntro: {
    id: 'app.products.intro',
    defaultMessage: 'Each stock product, and the medicine names it is prescribed by. The pharmacy uses them to know what to dispense.',
    description: 'Products: what the page is for',
  },
  pdNone: { id: 'app.products.none', defaultMessage: 'No products yet.', description: 'Products: empty' },
  pdFailed: { id: 'app.products.failed', defaultMessage: 'The products could not be loaded.', description: 'Products: failed to load' },
  pdListed: { id: 'app.products.listed', defaultMessage: '{name} ({code}), counted in {unit}', description: 'Products: one product' },
  pdSynonyms: { id: 'app.products.synonyms', defaultMessage: 'Also prescribed as: {names}', description: 'Products: other names' },
  pdAdd: { id: 'app.products.add', defaultMessage: 'Add a product', description: 'Products: heading of the form' },
  pdCode: { id: 'app.products.code', defaultMessage: 'Product code', description: 'Products: the stock code' },
  pdCodeHint: {
    id: 'app.products.codeHint',
    defaultMessage: 'Lowercase letters, digits and dashes, as on stock received. It never changes.',
    description: 'Products: hint for the code',
  },
  pdName: { id: 'app.products.name', defaultMessage: 'Name', description: 'Products: the name, as prescribed' },
  pdBaseUnit: { id: 'app.products.baseUnit', defaultMessage: 'Counted in (tablet, capsule, ml)', description: 'Products: the base unit' },
  pdPackSize: { id: 'app.products.packSize', defaultMessage: 'Pack size', description: 'Products: a pack size in base units' },
  pdSynonymsField: { id: 'app.products.synonymsField', defaultMessage: 'Other names (optional)', description: 'Products: other names field' },
  pdSynonymsHint: { id: 'app.products.synonymsHint', defaultMessage: 'Separate names with commas.', description: 'Products: hint for other names' },
  pdSave: { id: 'app.products.save', defaultMessage: 'Save product', description: 'Products action: save' },
  pdSaved: { id: 'app.products.saved', defaultMessage: 'Saved {name}.', description: 'Products: saved' },
  pdSaveFailed: { id: 'app.products.saveFailed', defaultMessage: 'That did not work. {reason}', description: 'Products: save failed' },
  pdEquipment: { id: 'app.products.equipment', defaultMessage: 'Tracked as equipment', description: 'Products: the product is equipment, not stock' },
  pdEquipmentHint: {
    id: 'app.products.equipmentHint',
    defaultMessage: 'Each unit is recorded by its serial number, not counted as stock. This cannot be changed later.',
    description: 'Products: hint for tracked as equipment',
  },
  pdIsEquipment: { id: 'app.products.isEquipment', defaultMessage: 'equipment', description: 'Products: listed product is equipment' },
  pdUsefulLife: { id: 'app.products.usefulLife', defaultMessage: 'Useful life in years (optional)', description: 'Products: years equipment depreciates over' },
  pdUsefulLifeHint: {
    id: 'app.products.usefulLifeHint',
    defaultMessage: 'Each unit loses value evenly over these years. Without it, a unit is valued at what it cost.',
    description: 'Products: hint for useful life',
  },
})

type EquipmentValues = {
  eqTitle: {}
  eqIntro: {}
  eqNone: {}
  eqFailed: {}
  eqCaption: {}
  eqName: {}
  eqSerial: {}
  eqLocation: {}
  eqStatus: {}
  eqLastChange: {}
  eqChange: { by: string; at: string; note: string }
  eqInTransit: { from: string }
  eqUnit: {}
  eqSelected: { name: string; serial: string }
  eqStatusField: {}
  eqNote: {}
  eqNoteHint: {}
  eqSetStatus: {}
  eqNewLocation: {}
  eqMove: {}
  eqFacility: {}
  eqFacilityHint: {}
  eqDispatch: {}
  eqReceiveInto: {}
  eqReceive: {}
  eqDone: { name: string; serial: string }
  eqRefused: { reason: string }
  eqRetired: {}
  eqAvailable: {}
  eqFaulty: {}
  eqUnderMaintenance: {}
  eqCost: {}
  eqBookValue: {}
  eqTotal: { value: string }
}

// Equipment (ADR 0064; #453): the facility's units, their status, moves and transfers (features/stock).
export const equipmentMessages = defineMessages<EquipmentValues>({
  eqTitle: { id: 'app.equipment.title', defaultMessage: 'Equipment', description: 'Equipment page heading and link' },
  eqIntro: {
    id: 'app.equipment.intro',
    defaultMessage: 'Each unit of equipment this facility holds, where it is and whether it works. Every change is kept as its history.',
    description: 'Equipment: what the page is for',
  },
  eqNone: { id: 'app.equipment.none', defaultMessage: 'This facility holds no equipment yet. Equipment is added by receiving goods.', description: 'Equipment: empty' },
  eqFailed: { id: 'app.equipment.failed', defaultMessage: 'The equipment could not be loaded.', description: 'Equipment: failed to load' },
  eqCaption: { id: 'app.equipment.caption', defaultMessage: 'Equipment held', description: 'Equipment: table caption' },
  eqName: { id: 'app.equipment.name', defaultMessage: 'Equipment', description: 'Equipment column: the product name' },
  eqSerial: { id: 'app.equipment.serial', defaultMessage: 'Serial number', description: 'Equipment column: serial number or asset tag' },
  eqLocation: { id: 'app.equipment.location', defaultMessage: 'Location', description: 'Equipment column: where it is' },
  eqStatus: { id: 'app.equipment.status', defaultMessage: 'Status', description: 'Equipment column: its status' },
  eqLastChange: { id: 'app.equipment.lastChange', defaultMessage: 'Last change', description: 'Equipment column: the latest change' },
  eqChange: { id: 'app.equipment.change', defaultMessage: '{note} ({by}, {at})', description: 'Equipment: the latest change' },
  eqInTransit: { id: 'app.equipment.inTransit', defaultMessage: 'In transit from {from}', description: 'Equipment: a unit being sent to this facility' },
  eqUnit: { id: 'app.equipment.unit', defaultMessage: 'Unit to change', description: 'Equipment: choose a unit to change' },
  eqSelected: { id: 'app.equipment.selected', defaultMessage: '{name}, serial number {serial}', description: 'Equipment: heading of the chosen unit' },
  eqStatusField: { id: 'app.equipment.statusField', defaultMessage: 'New status', description: 'Equipment: status field' },
  eqNote: { id: 'app.equipment.note', defaultMessage: 'What was done or found', description: 'Equipment: note of a status change' },
  eqNoteHint: { id: 'app.equipment.noteHint', defaultMessage: 'Needed unless it is available again.', description: 'Equipment: hint for the note' },
  eqSetStatus: { id: 'app.equipment.setStatus', defaultMessage: 'Save status', description: 'Equipment action: save status' },
  eqNewLocation: { id: 'app.equipment.newLocation', defaultMessage: 'Move to location', description: 'Equipment: location field to move to' },
  eqMove: { id: 'app.equipment.move', defaultMessage: 'Move', description: 'Equipment action: move' },
  eqFacility: { id: 'app.equipment.facility', defaultMessage: 'Send to facility (code)', description: 'Equipment: the facility to send a unit to' },
  eqFacilityHint: {
    id: 'app.equipment.facilityHint',
    defaultMessage: 'Once sent, it belongs to that facility and you can no longer change it.',
    description: 'Equipment: hint for sending to another facility',
  },
  eqDispatch: { id: 'app.equipment.dispatch', defaultMessage: 'Send', description: 'Equipment action: send to another facility' },
  eqReceiveInto: { id: 'app.equipment.receiveInto', defaultMessage: 'Receive into location', description: 'Equipment: location a unit in transit is received into' },
  eqReceive: { id: 'app.equipment.receive', defaultMessage: 'Receive', description: 'Equipment action: receive a unit in transit' },
  eqDone: { id: 'app.equipment.done', defaultMessage: 'Saved {name} {serial}.', description: 'Equipment: a change was saved' },
  eqRefused: { id: 'app.equipment.refused', defaultMessage: 'That did not work. {reason}', description: 'Equipment: a change was refused' },
  eqRetired: { id: 'app.equipment.retired', defaultMessage: 'Retired', description: 'Equipment status' },
  eqAvailable: { id: 'app.equipment.available', defaultMessage: 'Available', description: 'Equipment status' },
  eqFaulty: { id: 'app.equipment.faulty', defaultMessage: 'Faulty', description: 'Equipment status' },
  eqUnderMaintenance: { id: 'app.equipment.underMaintenance', defaultMessage: 'Under maintenance', description: 'Equipment status' },
  eqCost: { id: 'app.equipment.cost', defaultMessage: 'Cost', description: 'Equipment column: what a unit cost' },
  eqBookValue: { id: 'app.equipment.bookValue', defaultMessage: 'Book value', description: 'Equipment column: cost less depreciation' },
  eqTotal: { id: 'app.equipment.total', defaultMessage: 'Book value of the equipment held: {value}', description: 'Equipment: total book value' },
})

type TriageValues = {
  trTitle: {}
  trWaiting: {}
  trNoneWaiting: {}
  trTriage: { name: string }
  trWith: { name: string }
  trLoading: {}
  trNoForm: {}
  trDone: { name: string }
  trFailed: { reason: string }
  trBack: {}
}

// Triage's strings (features/triage, #103).
export const triageMessages = defineMessages<TriageValues>({
  trTitle: { id: 'app.triage.title', defaultMessage: 'Triage', description: 'Triage page heading and link' },
  trWaiting: { id: 'app.triage.waiting', defaultMessage: 'Waiting for triage', description: 'Triage: the queue heading' },
  trNoneWaiting: { id: 'app.triage.noneWaiting', defaultMessage: 'Nobody is waiting for triage.', description: 'Triage: empty queue' },
  trTriage: { id: 'app.triage.triage', defaultMessage: 'Triage {name}', description: 'Triage action: open the patient' },
  trWith: { id: 'app.triage.with', defaultMessage: 'Triage: {name}', description: 'Triage: the form heading' },
  trLoading: { id: 'app.triage.loading', defaultMessage: 'Loading the triage form…', description: 'Triage: the form is loading' },
  trNoForm: { id: 'app.triage.noForm', defaultMessage: 'The triage form is not available on this node.', description: 'Triage: no active triage Questionnaire' },
  trDone: { id: 'app.triage.done', defaultMessage: '{name} is triaged and waiting for consultation.', description: 'Triage: the triage was recorded' },
  trFailed: { id: 'app.triage.failed', defaultMessage: 'The triage was not recorded. {reason}', description: 'Triage: the save failed' },
  trBack: { id: 'app.triage.back', defaultMessage: 'Back to the queue', description: 'Triage action: close the form' },
})

type ConflictValues = {
  cfTitle: {}
  cfNone: {}
  cfLoading: {}
  cfRefused: {}
  cfOpen: { resource: string; peer: string; opened: string }
  cfOverdue: {}
  cfRecord: { resource: string }
  cfPolicy: { policy: string }
  cfHead: {}
  cfCandidate: { peer: string }
  cfNoCandidate: {}
  cfKeepHead: {}
  cfTakeCandidate: {}
  cfCorrected: {}
  cfWriteCorrected: {}
  cfNotJson: {}
  cfResolved: {}
  cfFailed: { reason: string }
  cfBack: {}
}

// The Conflict Record review queue's strings (features/conflicts, #82).
export const conflictMessages = defineMessages<ConflictValues>({
  cfTitle: { id: 'app.conflicts.title', defaultMessage: 'Sync conflicts', description: 'Conflict review page heading and link' },
  cfNone: { id: 'app.conflicts.none', defaultMessage: 'No conflicts are waiting for review.', description: 'Conflict review: empty queue' },
  cfLoading: { id: 'app.conflicts.loading', defaultMessage: 'Loading the conflicts…', description: 'Conflict review: loading' },
  cfRefused: { id: 'app.conflicts.refused', defaultMessage: 'You may not review sync conflicts.', description: 'Conflict review: no permission' },
  cfOpen: { id: 'app.conflicts.open', defaultMessage: 'Review {resource} from {peer}, opened {opened}', description: 'Conflict review action: open a record' },
  cfOverdue: { id: 'app.conflicts.overdue', defaultMessage: 'Overdue', description: 'Conflict review: open for more than 7 days' },
  cfRecord: { id: 'app.conflicts.record', defaultMessage: 'Conflict: {resource}', description: 'Conflict review: the record heading' },
  cfPolicy: { id: 'app.conflicts.policy', defaultMessage: 'Policy: {policy}', description: 'Conflict review: what the class policy says' },
  cfHead: { id: 'app.conflicts.head', defaultMessage: 'Current version here', description: 'Conflict review: the head branch' },
  cfCandidate: { id: 'app.conflicts.candidate', defaultMessage: 'Version from {peer}', description: 'Conflict review: the candidate branch' },
  cfNoCandidate: { id: 'app.conflicts.noCandidate', defaultMessage: 'No version was sent for this record.', description: 'Conflict review: missing candidate' },
  cfKeepHead: { id: 'app.conflicts.keepHead', defaultMessage: 'Keep the current version', description: 'Conflict review decision' },
  cfTakeCandidate: { id: 'app.conflicts.takeCandidate', defaultMessage: 'Use the facility’s version', description: 'Conflict review decision' },
  cfCorrected: { id: 'app.conflicts.corrected', defaultMessage: 'Corrected version (JSON)', description: 'Conflict review: the editable corrected version' },
  cfWriteCorrected: { id: 'app.conflicts.writeCorrected', defaultMessage: 'Save the corrected version', description: 'Conflict review decision' },
  cfNotJson: { id: 'app.conflicts.notJson', defaultMessage: 'The corrected version is not valid JSON.', description: 'Conflict review: bad corrected JSON' },
  cfResolved: { id: 'app.conflicts.resolved', defaultMessage: 'Resolved. The decision syncs to the facility.', description: 'Conflict review: decision recorded' },
  cfFailed: { id: 'app.conflicts.failed', defaultMessage: 'The decision was not recorded. {reason}', description: 'Conflict review: decision refused' },
  cfBack: { id: 'app.conflicts.back', defaultMessage: 'Back to the queue', description: 'Conflict review action' },
})

type StudioValues = {
  sdTitle: {}
  sdUnavailable: {}
  sdLoading: {}
  sdRefused: {}
  sdFailed: {}
  sdEmpty: {}
  sdHint: {}
  sdColPackage: {}
  sdColVersion: {}
  sdColStatus: {}
  sdColAuthor: {}
  sdColNext: {}
  sdColActive: {}
  sdVersionBump: { version: string; bump: string }
  sdVersionFirst: { version: string }
  sdNothing: {}
  sdNowhere: {}
  sdActiveIn: { scope: string; version: string }
  sdBack: {}
  sdPackageHeading: { title: string; version: string }
  sdSummary: { name: string; kind: string; stage: string; authors: string }
  sdHolder: { holder: string }
  sdOffered: { holder: string; to: string }
  sdHistoryPerson: { person: string }
  sdTo: {}
  sdToHint: {}
  sdWithdrawn: {}
  sdChanged: {}
  sdChangedAgainst: { version: string }
  sdChangedFirst: {}
  sdNoChanges: {}
  sdChangeAdded: { type: string; path: string; version: string }
  sdChangeChanged: { type: string; path: string; before: string; after: string }
  sdChangeRemoved: { type: string; path: string; version: string }
  sdSafe: { failing: number }
  sdNotChecked: {}
  sdCheckPassed: { check: string; at: string }
  sdCheckFailed: { check: string; at: string }
  sdCheckStale: {}
  sdWhoActs: {}
  sdApprovals: { given: number; required: number }
  sdReviewer: { reviewer: string }
  sdNoReviewer: {}
  sdApprovers: { approvers: string }
  sdDiscussion: {}
  sdHistoryEntry: { step: string; by: string; at: string }
  sdReason: {}
  sdScope: {}
  sdScopeHint: {}
  sdDone: {}
  sdActionFailed: { reason: string }
  sdRefusalPermission: { permission: string }
  sdRefusalStage: { stage: string }
  sdRefusalAuthor: {}
  sdRefusalNotHolder: {}
  sdRefusalNotOffered: {}
  sdRefusalAlreadyApproved: {}
  sdRefusalApprovalsNeeded: { missing: number }
  sdRefusalNotPublished: {}
  sdActionSubmit: {}
  sdActionReview: {}
  sdActionApprove: {}
  sdActionPublish: {}
  sdActionActivate: {}
  sdActionRequestChanges: {}
  sdStageDraft: {}
  sdStageValidation: {}
  sdStageSimulation: {}
  sdStageReview: {}
  sdStageApproval: {}
  sdStagePublished: {}
  sdCheckValidation: {}
  sdCheckSimulation: {}
  sdActionCheck: {}
  sdActionAcceptHandover: {}
  sdActionHandOver: {}
  sdActionDeclineHandover: {}
  sdActionWithdraw: {}
  sdActionReassign: {}
  sdStageWithdrawn: {}
  sdFiles: {}
  sdFilesHint: {}
  sdOpenFile: { path: string }
  sdNewFilePath: {}
  sdNewFilePathHint: {}
  sdAddFile: {}
  sdVersion: {}
  sdChangeVersion: {}
  sdVersionChanged: {}
  sdEditRefused: { reason: string }
  sdStartNext: {}
  sdStartNextHint: {}
  sdStartFailed: { reason: string }
  sdNewDraft: {}
  sdNewDraftName: {}
  sdNewDraftTitle: {}
  sdNewDraftKind: {}
  sdNewDraftVersion: {}
  sdNewDraftCanonical: {}
  sdNewDraftStart: {}
  sdEditorHeading: { path: string; id: string }
  sdEditorBack: {}
  sdEditorLoading: {}
  sdEditorNew: {}
  sdEditorContent: {}
  sdEditorSave: {}
  sdEditorSaved: {}
  sdEditorRemove: {}
  sdEditorRemoved: {}
  sdEditorStale: {}
  sdEditorReload: {}
  sdEditorNotJson: {}
  sdEditorReadOnly: {}
  sdEditorFailed: { reason: string }
  fdOpen: { path: string }
  fdOpenShort: {}
  fdHeading: { title: string }
  fdLoading: {}
  fdNotQuestionnaire: {}
  fdAsJson: {}
  fdQuestions: {}
  fdNoSection: {}
  fdAddQuestion: {}
  fdNewQuestion: {}
  fdUp: { text: string }
  fdDown: { text: string }
  fdRemove: { text: string }
  fdUntitled: { id: string }
  fdSections: {}
  fdSectionTitle: { n: number }
  fdAddSection: {}
  fdNewSection: { n: number }
  fdSectionUp: { title: string }
  fdSectionDown: { title: string }
  fdSectionRemove: { title: string }
  fdInspector: {}
  fdChoose: {}
  fdQuestion: {}
  fdText: {}
  fdId: {}
  fdIdFixed: {}
  fdIdNew: {}
  fdType: {}
  fdTypeOther: { type: string }
  fdRequired: {}
  fdRepeats: {}
  fdOptions: {}
  fdOptionCode: { n: number }
  fdOptionDisplay: { n: number }
  fdOptionUp: { n: number }
  fdOptionDown: { n: number }
  fdOptionRemove: { n: number }
  fdAddOption: {}
  fdNewOption: { n: number }
  fdValueSet: { valueSet: string }
  fdBindValueSet: {}
  fdChangeValueSet: {}
  fdUnbind: {}
  fdPicker: {}
  fdPickerSearch: {}
  fdPickerHint: {}
  fdPickerResults: {}
  fdPickerNone: {}
  fdPickerFailed: { reason: string }
  fdPickerOwn: {}
  fdPickerPublished: { package: string }
  fdPickerCancel: {}
  fdBind: { title: string }
  fdBindNone: {}
  fdBindReplaces: {}
  fdCodesLoading: {}
  fdCodesOf: { binding: string }
  fdCodesMore: { more: number }
  fdWidgetName: { widget: string }
  fdSuggested: { widget: string }
  fdSuggestedSearch: {}
  fdUseSuggested: { widget: string }
  fdShownWhen: {}
  fdAlways: {}
  fdIsYes: { text: string }
  fdConditionOther: {}
  fdLayout: {}
  fdSection: {}
  fdSectionNone: {}
  fdSectionNested: {}
  fdWidget: {}
  fdWidgetDefault: { widget: string }
  fdHint: {}
  fdRecord: {}
  fdRecordNone: {}
  fdRecordTarget: { n: number; type: string }
  fdRecordSystem: { n: number }
  fdRecordCode: { n: number }
  fdRecordDisplay: { n: number }
  fdRecordRemove: { n: number }
  fdRecordAdd: {}
  fdRecordNotRecordable: { type: string }
  fdMappingFile: { path: string }
  fdMappingNew: { path: string }
  fdPRecordNoCode: { text: string }
  fdPRecordOrphan: { id: string }
  fdImport: {}
  fdImportHeading: {}
  fdImportJson: {}
  fdImportVersion: {}
  fdImportR5: {}
  fdImportR4: {}
  fdImportRead: {}
  fdImportCancel: {}
  fdImportReplace: {}
  fdImportNotJson: {}
  fdImportNotQuestionnaire: {}
  fdImportLooksR4: {}
  fdImportReport: { count: number }
  fdImportBlocked: {}
  fdImportClean: {}
  fdImportReplaced: {}
  fdRChoice: { at: string }
  fdROpenChoice: { at: string }
  fdRSource: { source: string }
  fdRUnknown: { at: string; element: string }
  fdRContained: {}
  fdRModifier: { at: string }
  fdRType: { at: string; type: string }
  fdPreview: {}
  fdProblems: { count: number }
  fdProblemsHint: {}
  fdPNoId: {}
  fdPDuplicate: { id: string }
  fdPNoText: { id: string }
  fdPNoOptions: { text: string }
  fdPCondition: { text: string }
  fdPWidget: { text: string; widget: string }
  fdPNoSection: { text: string }
  fdPRemoved: { id: string }
  fdPSectionTitle: { n: number }
  fdSave: {}
  fdSaved: {}
  fdUnsaved: {}
  fdLayoutFile: { path: string }
  fdLayoutNew: { path: string }
}

// The Studio's package list and draft page (features/studio, #492).
export const studioMessages = defineMessages<StudioValues>({
  sdTitle: { id: 'app.studio.title', defaultMessage: 'Studio', description: 'Studio package list heading and link' },
  sdUnavailable: { id: 'app.studio.unavailable', defaultMessage: 'The Studio is only available at central.', description: 'Studio opened on an edge or a desktop' },
  sdLoading: { id: 'app.studio.loading', defaultMessage: 'Loading the packages…', description: 'Studio: loading' },
  sdRefused: { id: 'app.studio.refused', defaultMessage: 'You need a metadata permission to use the Studio.', description: 'Studio: no permission' },
  sdFailed: { id: 'app.studio.failed', defaultMessage: 'The Studio could not be loaded.', description: 'Studio: request failed' },
  sdEmpty: { id: 'app.studio.empty', defaultMessage: 'There are no packages yet.', description: 'Studio: empty package list' },
  sdHint: { id: 'app.studio.hint', defaultMessage: 'Click a package to open it, or move with the arrow keys and press Enter.', description: 'Studio: how to use the package table' },
  sdColPackage: { id: 'app.studio.colPackage', defaultMessage: 'Package', description: 'Studio table column' },
  sdColVersion: { id: 'app.studio.colVersion', defaultMessage: 'Version', description: 'Studio table column' },
  sdColStatus: { id: 'app.studio.colStatus', defaultMessage: 'Status', description: 'Studio table column' },
  sdColAuthor: { id: 'app.studio.colAuthor', defaultMessage: 'Held by', description: 'Studio table column: the Draft Holder' },
  sdColNext: { id: 'app.studio.colNext', defaultMessage: 'Your next step', description: 'Studio table column' },
  sdColActive: { id: 'app.studio.colActive', defaultMessage: 'Active in', description: 'Studio table column' },
  sdVersionBump: { id: 'app.studio.versionBump', defaultMessage: '{version} ({bump})', description: 'Studio: a version and its semver bump' },
  sdVersionFirst: { id: 'app.studio.versionFirst', defaultMessage: '{version} (first version)', description: 'Studio: a first version' },
  sdNothing: { id: 'app.studio.nothing', defaultMessage: 'Nothing for you', description: 'Studio: no next step for the signed-in person' },
  sdNowhere: { id: 'app.studio.nowhere', defaultMessage: 'Not active', description: 'Studio: active in no scope' },
  sdActiveIn: { id: 'app.studio.activeIn', defaultMessage: '{scope} ({version})', description: 'Studio: a scope and the version active there' },
  sdBack: { id: 'app.studio.back', defaultMessage: 'Back to the packages', description: 'Studio: back to the list' },
  sdPackageHeading: { id: 'app.studio.packageHeading', defaultMessage: '{title} {version}', description: 'Studio draft page heading' },
  sdSummary: { id: 'app.studio.summary', defaultMessage: '{name}, {kind} package, {stage}. Held so far by {authors}.', description: 'Studio draft page summary; authors are everyone who has held the draft' },
  sdHolder: { id: 'app.studio.holder', defaultMessage: 'Held by {holder}.', description: 'Studio: the Draft Holder' },
  sdOffered: { id: 'app.studio.offered', defaultMessage: 'Held by {holder}, who is handing it over to {to}. It stays with {holder} until {to} accepts.', description: 'Studio: a handover waiting to be accepted' },
  sdHistoryPerson: { id: 'app.studio.historyPerson', defaultMessage: 'Concerning {person}', description: 'Studio: who a handover or reassignment concerns' },
  sdTo: { id: 'app.studio.to', defaultMessage: 'Hand over or reassign to', description: 'Studio: who a draft goes to, by username' },
  sdToHint: { id: 'app.studio.toHint', defaultMessage: 'Their username. A handover waits for them to accept; a reassignment needs a reason, above.', description: 'Studio: how to name who a draft goes to' },
  sdWithdrawn: { id: 'app.studio.withdrawn', defaultMessage: 'This draft was withdrawn. It is kept with its history, and its version can be drafted again.', description: 'Studio: a withdrawn draft' },
  sdChanged: { id: 'app.studio.changed', defaultMessage: 'What changed', description: 'Studio draft page section' },
  sdChangedAgainst: { id: 'app.studio.changedAgainst', defaultMessage: 'Compared with the published version {version}.', description: 'Studio: what the diff is against' },
  sdChangedFirst: { id: 'app.studio.changedFirst', defaultMessage: 'This is the first version: every artefact is new.', description: 'Studio: first version diff' },
  sdNoChanges: { id: 'app.studio.noChanges', defaultMessage: 'No artefact changed.', description: 'Studio: empty diff' },
  sdChangeAdded: { id: 'app.studio.changeAdded', defaultMessage: 'Added {type} {path}, version {version}', description: 'Studio: an added artefact' },
  sdChangeChanged: { id: 'app.studio.changeChanged', defaultMessage: 'Changed {type} {path}, version {before} to {after}', description: 'Studio: a changed artefact' },
  sdChangeRemoved: { id: 'app.studio.changeRemoved', defaultMessage: 'Removed {type} {path}, version {version}', description: 'Studio: a removed artefact' },
  sdSafe: { id: 'app.studio.safe', defaultMessage: 'Is it safe? {failing, plural, =0 {Nothing failing} one {# check failing} other {# checks failing}}', description: 'Studio draft page section with the failing count' },
  sdNotChecked: { id: 'app.studio.notChecked', defaultMessage: 'Not checked yet: submitting runs validation and simulation.', description: 'Studio: no check results' },
  sdCheckPassed: { id: 'app.studio.checkPassed', defaultMessage: '{check} passed, {at}', description: 'Studio: a passing check' },
  sdCheckFailed: { id: 'app.studio.checkFailed', defaultMessage: '{check} failed, {at}', description: 'Studio: a failing check' },
  sdCheckStale: { id: 'app.studio.checkStale', defaultMessage: 'The draft changed since this ran.', description: 'Studio: a check result on older content' },
  sdWhoActs: { id: 'app.studio.whoActs', defaultMessage: 'Who must act', description: 'Studio draft page section' },
  sdApprovals: { id: 'app.studio.approvals', defaultMessage: '{given} of {required} approvals', description: 'Studio: approvals given and needed' },
  sdReviewer: { id: 'app.studio.reviewer', defaultMessage: 'Reviewed by {reviewer}', description: 'Studio: who reviewed' },
  sdNoReviewer: { id: 'app.studio.noReviewer', defaultMessage: 'Not reviewed yet', description: 'Studio: no reviewer yet' },
  sdApprovers: { id: 'app.studio.approvers', defaultMessage: 'Approved by {approvers}', description: 'Studio: who approved' },
  sdDiscussion: { id: 'app.studio.discussion', defaultMessage: 'Discussion', description: 'Studio draft page section' },
  sdHistoryEntry: { id: 'app.studio.historyEntry', defaultMessage: '{step} by {by}, {at}', description: 'Studio: one history entry' },
  sdReason: { id: 'app.studio.reason', defaultMessage: 'Reason: what needs to change, or why the draft is reassigned', description: 'Studio: request changes or reassign reason field' },
  sdScope: { id: 'app.studio.scope', defaultMessage: 'Activate in', description: 'Studio: activation scope field' },
  sdScopeHint: { id: 'app.studio.scopeHint', defaultMessage: 'national, district:<code>, facility:<code> or program:<facility>:<program>', description: 'Studio: how to write a scope' },
  sdDone: { id: 'app.studio.done', defaultMessage: 'Done.', description: 'Studio: a step was taken' },
  sdActionFailed: { id: 'app.studio.actionFailed', defaultMessage: 'The step was not taken. {reason}', description: 'Studio: a step was refused' },
  sdRefusalPermission: { id: 'app.studio.refusalPermission', defaultMessage: 'You need the {permission} permission', description: 'Studio: refused, missing permission' },
  sdRefusalStage: { id: 'app.studio.refusalStage', defaultMessage: 'Not possible while it is at the {stage} stage', description: 'Studio: refused at this stage' },
  sdRefusalAuthor: { id: 'app.studio.refusalAuthor', defaultMessage: 'You wrote this draft, so you cannot review or approve it', description: 'Studio: refused, separation of duties' },
  sdRefusalNotHolder: { id: 'app.studio.refusalNotHolder', defaultMessage: "Only the draft's holder may do that", description: 'Studio: refused, not the Draft Holder' },
  sdRefusalNotOffered: { id: 'app.studio.refusalNotOffered', defaultMessage: 'No handover of this draft is waiting for you', description: 'Studio: refused, no handover offered' },
  sdRefusalAlreadyApproved: { id: 'app.studio.refusalAlreadyApproved', defaultMessage: 'You have already approved this draft', description: 'Studio: refused, approved already' },
  sdRefusalApprovalsNeeded: { id: 'app.studio.refusalApprovalsNeeded', defaultMessage: '{missing, plural, one {# more approval needed} other {# more approvals needed}}', description: 'Studio: refused, approvals missing' },
  sdRefusalNotPublished: { id: 'app.studio.refusalNotPublished', defaultMessage: 'Only a published version can be activated', description: 'Studio: refused, not published' },
  sdActionSubmit: { id: 'app.studio.actionSubmit', defaultMessage: 'Submit', description: 'Studio action' },
  sdActionReview: { id: 'app.studio.actionReview', defaultMessage: 'Accept the review', description: 'Studio action' },
  sdActionApprove: { id: 'app.studio.actionApprove', defaultMessage: 'Approve', description: 'Studio action' },
  sdActionPublish: { id: 'app.studio.actionPublish', defaultMessage: 'Publish', description: 'Studio action' },
  sdActionActivate: { id: 'app.studio.actionActivate', defaultMessage: 'Activate', description: 'Studio action' },
  sdActionRequestChanges: { id: 'app.studio.actionRequestChanges', defaultMessage: 'Request changes', description: 'Studio action' },
  sdStageDraft: { id: 'app.studio.stageDraft', defaultMessage: 'draft', description: 'Studio: lifecycle stage' },
  sdStageValidation: { id: 'app.studio.stageValidation', defaultMessage: 'validation', description: 'Studio: lifecycle stage' },
  sdStageSimulation: { id: 'app.studio.stageSimulation', defaultMessage: 'simulation', description: 'Studio: lifecycle stage' },
  sdStageReview: { id: 'app.studio.stageReview', defaultMessage: 'review', description: 'Studio: lifecycle stage' },
  sdStageApproval: { id: 'app.studio.stageApproval', defaultMessage: 'approval', description: 'Studio: lifecycle stage' },
  sdStagePublished: { id: 'app.studio.stagePublished', defaultMessage: 'published', description: 'Studio: lifecycle stage' },
  sdCheckValidation: { id: 'app.studio.checkValidation', defaultMessage: 'Validation', description: 'Studio: check name' },
  sdCheckSimulation: { id: 'app.studio.checkSimulation', defaultMessage: 'Simulation', description: 'Studio: check name' },
  sdActionCheck: { id: 'app.studio.actionCheck', defaultMessage: 'Check now', description: 'Studio action: run validation and simulation without submitting' },
  sdActionAcceptHandover: { id: 'app.studio.actionAcceptHandover', defaultMessage: 'Accept the handover', description: 'Studio action: become the Draft Holder' },
  sdActionHandOver: { id: 'app.studio.actionHandOver', defaultMessage: 'Hand over', description: 'Studio action: offer the draft to someone' },
  sdActionDeclineHandover: { id: 'app.studio.actionDeclineHandover', defaultMessage: 'Call off the handover', description: 'Studio action: cancel or decline a handover' },
  sdActionWithdraw: { id: 'app.studio.actionWithdraw', defaultMessage: 'Withdraw', description: 'Studio action: withdraw the draft' },
  sdActionReassign: { id: 'app.studio.actionReassign', defaultMessage: 'Reassign', description: 'Studio action: give the draft to someone else, as a manager' },
  sdStageWithdrawn: { id: 'app.studio.stageWithdrawn', defaultMessage: 'withdrawn', description: 'Studio: lifecycle stage' },
  sdFiles: { id: 'app.studio.files', defaultMessage: 'Files', description: 'Studio: the draft files heading' },
  sdFilesHint: { id: 'app.studio.filesHint', defaultMessage: 'Open a file to read it or, in a draft, to edit it. Each save is one file.', description: 'Studio: files section hint' },
  sdOpenFile: { id: 'app.studio.openFile', defaultMessage: 'Open {path}', description: 'Studio: link to a file editor' },
  sdNewFilePath: { id: 'app.studio.newFilePath', defaultMessage: 'New file', description: 'Studio: path of a file to add' },
  sdNewFilePathHint: { id: 'app.studio.newFilePathHint', defaultMessage: 'For example ValueSet-anc-risk.json, or tests/visit.json', description: 'Studio: how to name a new file' },
  sdAddFile: { id: 'app.studio.addFile', defaultMessage: 'Add the file', description: 'Studio: open the editor for a new file' },
  sdVersion: { id: 'app.studio.version', defaultMessage: 'Version', description: 'Studio: the draft version field' },
  sdChangeVersion: { id: 'app.studio.changeVersion', defaultMessage: 'Change the version', description: 'Studio: action' },
  sdVersionChanged: { id: 'app.studio.versionChanged', defaultMessage: 'The draft has its new version.', description: 'Studio: version changed' },
  sdEditRefused: { id: 'app.studio.editRefused', defaultMessage: 'You cannot change this package: {reason}', description: 'Studio: why editing is off' },
  sdStartNext: { id: 'app.studio.startNext', defaultMessage: 'Start the next version', description: 'Studio: copy a published version into a new draft' },
  sdStartNextHint: { id: 'app.studio.startNextHint', defaultMessage: 'A draft of the next minor version, with the same artefacts. You can change its version until you submit it.', description: 'Studio: what starting the next version does' },
  sdStartFailed: { id: 'app.studio.startFailed', defaultMessage: 'The draft was not started. {reason}', description: 'Studio: starting a draft failed' },
  sdNewDraft: { id: 'app.studio.newDraft', defaultMessage: 'Start a new package', description: 'Studio: blank draft heading' },
  sdNewDraftName: { id: 'app.studio.newDraftName', defaultMessage: 'Name', description: 'Studio: new package name, e.g. ug.moh.tb' },
  sdNewDraftTitle: { id: 'app.studio.newDraftTitle', defaultMessage: 'Title', description: 'Studio: new package title' },
  sdNewDraftKind: { id: 'app.studio.newDraftKind', defaultMessage: 'Kind', description: 'Studio: new package kind' },
  sdNewDraftVersion: { id: 'app.studio.newDraftVersion', defaultMessage: 'First version', description: 'Studio: new package version' },
  sdNewDraftCanonical: { id: 'app.studio.newDraftCanonical', defaultMessage: 'Canonical base URL', description: 'Studio: new package canonical base' },
  sdNewDraftStart: { id: 'app.studio.newDraftStart', defaultMessage: 'Start the draft', description: 'Studio: start a blank draft' },
  sdEditorHeading: { id: 'app.studio.editorHeading', defaultMessage: '{path} in {id}', description: 'Studio: file editor heading' },
  sdEditorBack: { id: 'app.studio.editorBack', defaultMessage: 'Back to the package', description: 'Studio: file editor back link' },
  sdEditorLoading: { id: 'app.studio.editorLoading', defaultMessage: 'Loading the file…', description: 'Studio: file editor loading' },
  sdEditorNew: { id: 'app.studio.editorNew', defaultMessage: 'This file is new: it is added to the draft when you save it.', description: 'Studio: editing a file that does not exist yet' },
  sdEditorContent: { id: 'app.studio.editorContent', defaultMessage: 'Content (JSON)', description: 'Studio: file editor text area' },
  sdEditorSave: { id: 'app.studio.editorSave', defaultMessage: 'Save', description: 'Studio: save the file' },
  sdEditorSaved: { id: 'app.studio.editorSaved', defaultMessage: 'Saved.', description: 'Studio: file saved' },
  sdEditorRemove: { id: 'app.studio.editorRemove', defaultMessage: 'Remove the file', description: 'Studio: remove the file from the draft' },
  sdEditorRemoved: { id: 'app.studio.editorRemoved', defaultMessage: 'Removed.', description: 'Studio: file removed' },
  sdEditorStale: { id: 'app.studio.editorStale', defaultMessage: 'Someone saved this draft after you opened it, so your change was not saved. Reload to see the draft as it is now; copy your text first if you need it.', description: 'Studio: a save was refused with 412' },
  sdEditorReload: { id: 'app.studio.editorReload', defaultMessage: 'Reload', description: 'Studio: reload the file after a refused save' },
  sdEditorNotJson: { id: 'app.studio.editorNotJson', defaultMessage: 'This is not valid JSON.', description: 'Studio: file editor parse error' },
  sdEditorReadOnly: { id: 'app.studio.editorReadOnly', defaultMessage: 'Read only.', description: 'Studio: the file cannot be edited by this person or at this stage' },
  sdEditorFailed: { id: 'app.studio.editorFailed', defaultMessage: 'Not saved. {reason}', description: 'Studio: a save was refused' },
  fdOpen: { id: 'app.studio.formDesigner.open', defaultMessage: '{path} in the form designer', description: 'Studio: link from a Questionnaire to the form designer' },
  fdOpenShort: { id: 'app.studio.formDesigner.openShort', defaultMessage: 'Form designer', description: 'Studio: visible text of the link to the form designer' },
  fdHeading: { id: 'app.studio.formDesigner.heading', defaultMessage: 'Form designer: {title}', description: 'Studio: form designer heading' },
  fdLoading: { id: 'app.studio.formDesigner.loading', defaultMessage: 'Loading the form…', description: 'Studio: form designer loading' },
  fdNotQuestionnaire: { id: 'app.studio.formDesigner.notQuestionnaire', defaultMessage: 'This file is not a Questionnaire. Open it as JSON instead.', description: 'Studio: form designer opened on another file' },
  fdAsJson: { id: 'app.studio.formDesigner.asJson', defaultMessage: 'Open as JSON', description: 'Studio: form designer link to the JSON editor' },
  fdQuestions: { id: 'app.studio.formDesigner.questions', defaultMessage: 'Questions', description: 'Studio: form designer outline heading' },
  fdNoSection: { id: 'app.studio.formDesigner.noSection', defaultMessage: 'In no section: shown at the end of the form', description: 'Studio: outline group of questions no section places' },
  fdAddQuestion: { id: 'app.studio.formDesigner.addQuestion', defaultMessage: 'Add question', description: 'Studio: form designer button' },
  fdNewQuestion: { id: 'app.studio.formDesigner.newQuestion', defaultMessage: 'New question', description: 'Studio: text of a newly added question' },
  fdUp: { id: 'app.studio.formDesigner.up', defaultMessage: 'Move {text} up', description: 'Studio: move a question up' },
  fdDown: { id: 'app.studio.formDesigner.down', defaultMessage: 'Move {text} down', description: 'Studio: move a question down' },
  fdRemove: { id: 'app.studio.formDesigner.remove', defaultMessage: 'Remove {text}', description: 'Studio: remove a question' },
  fdUntitled: { id: 'app.studio.formDesigner.untitled', defaultMessage: '(no text) {id}', description: 'Studio: a question without text in the outline' },
  fdSections: { id: 'app.studio.formDesigner.sections', defaultMessage: 'Sections', description: 'Studio: form designer sections heading' },
  fdSectionTitle: { id: 'app.studio.formDesigner.sectionTitle', defaultMessage: 'Title of section {n}', description: 'Studio: section title field' },
  fdAddSection: { id: 'app.studio.formDesigner.addSection', defaultMessage: 'Add section', description: 'Studio: form designer button' },
  fdNewSection: { id: 'app.studio.formDesigner.newSection', defaultMessage: 'Section {n}', description: 'Studio: title of a new section' },
  fdSectionUp: { id: 'app.studio.formDesigner.sectionUp', defaultMessage: 'Move section {title} up', description: 'Studio: move a section' },
  fdSectionDown: { id: 'app.studio.formDesigner.sectionDown', defaultMessage: 'Move section {title} down', description: 'Studio: move a section' },
  fdSectionRemove: { id: 'app.studio.formDesigner.sectionRemove', defaultMessage: 'Remove section {title}', description: 'Studio: remove a section; its questions are in none' },
  fdInspector: { id: 'app.studio.formDesigner.inspector', defaultMessage: 'Selected question', description: 'Studio: inspector heading' },
  fdChoose: { id: 'app.studio.formDesigner.choose', defaultMessage: 'Choose a question in the list.', description: 'Studio: inspector with nothing selected' },
  fdQuestion: { id: 'app.studio.formDesigner.question', defaultMessage: 'Question (clinical: approved with the package)', description: 'Studio: inspector group for the Questionnaire' },
  fdText: { id: 'app.studio.formDesigner.text', defaultMessage: 'Text', description: 'Studio: question text field' },
  fdId: { id: 'app.studio.formDesigner.id', defaultMessage: 'Id', description: 'Studio: question id field' },
  fdIdFixed: { id: 'app.studio.formDesigner.idFixed', defaultMessage: 'Published: the id is fixed, because recorded answers and mappings refer to it.', description: 'Studio: why a published question’s id cannot change' },
  fdIdNew: { id: 'app.studio.formDesigner.idNew', defaultMessage: 'Fixed once published.', description: 'Studio: hint on a new question’s id' },
  fdType: { id: 'app.studio.formDesigner.type', defaultMessage: 'Answer type', description: 'Studio: answer type field' },
  fdTypeOther: { id: 'app.studio.formDesigner.typeOther', defaultMessage: '{type}: edited as JSON for now', description: 'Studio: an item type the designer does not edit' },
  fdRequired: { id: 'app.studio.formDesigner.required', defaultMessage: 'Required', description: 'Studio: required checkbox' },
  fdRepeats: { id: 'app.studio.formDesigner.repeats', defaultMessage: 'More than one answer', description: 'Studio: repeats checkbox' },
  fdOptions: { id: 'app.studio.formDesigner.options', defaultMessage: 'Options', description: 'Studio: answer options heading' },
  fdOptionCode: { id: 'app.studio.formDesigner.optionCode', defaultMessage: 'Code of option {n}', description: 'Studio: option code field' },
  fdOptionDisplay: { id: 'app.studio.formDesigner.optionDisplay', defaultMessage: 'Option {n}', description: 'Studio: option display field' },
  fdOptionUp: { id: 'app.studio.formDesigner.optionUp', defaultMessage: 'Move option {n} up', description: 'Studio: move an option' },
  fdOptionDown: { id: 'app.studio.formDesigner.optionDown', defaultMessage: 'Move option {n} down', description: 'Studio: move an option' },
  fdOptionRemove: { id: 'app.studio.formDesigner.optionRemove', defaultMessage: 'Remove option {n}', description: 'Studio: remove an option' },
  fdAddOption: { id: 'app.studio.formDesigner.addOption', defaultMessage: 'Add option', description: 'Studio: add an answer option' },
  fdNewOption: { id: 'app.studio.formDesigner.newOption', defaultMessage: 'Option {n}', description: 'Studio: display of a new option' },
  fdValueSet: { id: 'app.studio.formDesigner.valueSet', defaultMessage: 'Bound to the ValueSet {valueSet}', description: 'Studio: an item bound to a ValueSet' },
  fdBindValueSet: { id: 'app.studio.formDesigner.bindValueSet', defaultMessage: 'Bind a ValueSet', description: 'Studio: open the ValueSet picker' },
  fdChangeValueSet: { id: 'app.studio.formDesigner.changeValueSet', defaultMessage: 'Bind another ValueSet', description: 'Studio: open the ValueSet picker for a bound question' },
  fdUnbind: { id: 'app.studio.formDesigner.unbind', defaultMessage: 'Unbind the ValueSet', description: 'Studio: remove a question’s ValueSet binding' },
  fdPicker: { id: 'app.studio.formDesigner.picker', defaultMessage: 'ValueSet picker', description: 'Studio: the ValueSet picker group' },
  fdPickerSearch: { id: 'app.studio.formDesigner.pickerSearch', defaultMessage: 'Search ValueSets', description: 'Studio: search ValueSets to bind' },
  fdPickerHint: { id: 'app.studio.formDesigner.pickerHint', defaultMessage: 'Published ValueSets, and drafts in this package, by URL, name or title.', description: 'Studio: which ValueSets the picker finds' },
  fdPickerResults: { id: 'app.studio.formDesigner.pickerResults', defaultMessage: 'ValueSets found', description: 'Studio: the ValueSets the picker found' },
  fdPickerNone: { id: 'app.studio.formDesigner.pickerNone', defaultMessage: 'No ValueSet matches.', description: 'Studio: the picker found nothing' },
  fdPickerFailed: { id: 'app.studio.formDesigner.pickerFailed', defaultMessage: 'The ValueSets cannot be read: {reason}', description: 'Studio: the picker could not search' },
  fdPickerOwn: { id: 'app.studio.formDesigner.pickerOwn', defaultMessage: 'Draft in this package', description: 'Studio: a ValueSet of the package being edited' },
  fdPickerPublished: { id: 'app.studio.formDesigner.pickerPublished', defaultMessage: 'Published in {package}', description: 'Studio: a published ValueSet and its package' },
  fdPickerCancel: { id: 'app.studio.formDesigner.pickerCancel', defaultMessage: 'Cancel', description: 'Studio: close the ValueSet picker' },
  fdBind: { id: 'app.studio.formDesigner.bind', defaultMessage: 'Bind {title}', description: 'Studio: bind the chosen ValueSet' },
  fdBindNone: { id: 'app.studio.formDesigner.bindNone', defaultMessage: 'Bind', description: 'Studio: bind, before a ValueSet is chosen' },
  fdBindReplaces: { id: 'app.studio.formDesigner.bindReplaces', defaultMessage: 'Binding replaces the question’s own options with the ValueSet’s codes.', description: 'Studio: binding drops the answer options' },
  fdCodesLoading: { id: 'app.studio.formDesigner.codesLoading', defaultMessage: 'Reading…', description: 'Studio: the picker is reading' },
  fdCodesOf: { id: 'app.studio.formDesigner.codesOf', defaultMessage: 'Codes of {binding}', description: 'Studio: the codes previewed for a ValueSet' },
  fdCodesMore: { id: 'app.studio.formDesigner.codesMore', defaultMessage: 'and {more, plural, one {# more code} other {# more codes}}', description: 'Studio: codes not previewed' },
  fdWidgetName: { id: 'app.studio.formDesigner.widgetName', defaultMessage: '{widget, select, radio {radio buttons} select {a drop-down} other {search as you type}}', description: 'Studio: a widget, named' },
  fdSuggested: { id: 'app.studio.formDesigner.suggested', defaultMessage: 'Suggested widget: {widget}', description: 'Studio: the widget a ValueSet’s size suggests' },
  fdSuggestedSearch: { id: 'app.studio.formDesigner.suggestedSearch', defaultMessage: 'A Form Layout has no search-as-you-type widget yet, so it shows as a drop-down.', description: 'Studio: no search widget yet' },
  fdUseSuggested: { id: 'app.studio.formDesigner.useSuggested', defaultMessage: 'Show it as {widget}', description: 'Studio: apply the suggested widget in the layout' },
  fdShownWhen: { id: 'app.studio.formDesigner.shownWhen', defaultMessage: 'Shown only when', description: 'Studio: enableWhen field' },
  fdAlways: { id: 'app.studio.formDesigner.always', defaultMessage: 'Always shown', description: 'Studio: no condition' },
  fdIsYes: { id: 'app.studio.formDesigner.isYes', defaultMessage: '“{text}” is Yes', description: 'Studio: condition on a yes/no question' },
  fdConditionOther: { id: 'app.studio.formDesigner.conditionOther', defaultMessage: 'Its conditions are edited as JSON for now, and kept as they are.', description: 'Studio: conditions the designer does not edit' },
  fdLayout: { id: 'app.studio.formDesigner.layout', defaultMessage: 'Layout (presentation)', description: 'Studio: inspector group for the Form Layout' },
  fdSection: { id: 'app.studio.formDesigner.section', defaultMessage: 'Section', description: 'Studio: section field' },
  fdSectionNone: { id: 'app.studio.formDesigner.sectionNone', defaultMessage: 'No section (shown at the end)', description: 'Studio: no section' },
  fdSectionNested: { id: 'app.studio.formDesigner.sectionNested', defaultMessage: 'In the section of the group it belongs to.', description: 'Studio: a nested question’s section' },
  fdWidget: { id: 'app.studio.formDesigner.widget', defaultMessage: 'Shown as', description: 'Studio: widget field' },
  fdWidgetDefault: { id: 'app.studio.formDesigner.widgetDefault', defaultMessage: 'Default ({widget})', description: 'Studio: the type’s default widget' },
  fdHint: { id: 'app.studio.formDesigner.hint', defaultMessage: 'Hint', description: 'Studio: hint field' },
  fdRecord: { id: 'app.studio.formDesigner.record', defaultMessage: 'Becomes a record', description: 'Studio: inspector group for extraction' },
  fdRecordNone: { id: 'app.studio.formDesigner.recordNone', defaultMessage: 'No record: the answer stays in the response.', description: 'Studio: question with no extraction target' },
  fdRecordTarget: { id: 'app.studio.formDesigner.recordTarget', defaultMessage: 'Record {n}: {type}', description: 'Studio: extraction target resource type' },
  fdRecordSystem: { id: 'app.studio.formDesigner.recordSystem', defaultMessage: 'Code system of record {n}', description: 'Studio: extraction code system field' },
  fdRecordCode: { id: 'app.studio.formDesigner.recordCode', defaultMessage: 'Code of record {n}', description: 'Studio: extraction code field' },
  fdRecordDisplay: { id: 'app.studio.formDesigner.recordDisplay', defaultMessage: 'Display of record {n}', description: 'Studio: extraction code display field' },
  fdRecordRemove: { id: 'app.studio.formDesigner.recordRemove', defaultMessage: 'Remove record {n}', description: 'Studio: remove an extraction target' },
  fdRecordAdd: { id: 'app.studio.formDesigner.recordAdd', defaultMessage: 'Record it as an Observation', description: 'Studio: add an Observation extraction target' },
  fdRecordNotRecordable: { id: 'app.studio.formDesigner.recordNotRecordable', defaultMessage: 'A {type} item does not become a record itself.', description: 'Studio: an item type with no Observation value' },
  fdMappingFile: { id: 'app.studio.formDesigner.mappingFile', defaultMessage: 'What answers become is in {path}.', description: 'Studio: the Extraction Mapping file' },
  fdMappingNew: { id: 'app.studio.formDesigner.mappingNew', defaultMessage: 'It has no Extraction Mapping yet: recording an answer adds {path}.', description: 'Studio: no Extraction Mapping yet' },
  fdPRecordNoCode: { id: 'app.studio.formDesigner.pRecordNoCode', defaultMessage: 'What “{text}” becomes has no code', description: 'Studio problem' },
  fdPRecordOrphan: { id: 'app.studio.formDesigner.pRecordOrphan', defaultMessage: 'The Extraction Mapping records “{id}”, which is no longer in the form', description: 'Studio problem' },
  fdImport: { id: 'app.studio.formDesigner.import', defaultMessage: 'Import FHIR JSON', description: 'Studio: open the import panel' },
  fdImportHeading: { id: 'app.studio.formDesigner.importHeading', defaultMessage: 'Import FHIR JSON', description: 'Studio: import panel heading' },
  fdImportJson: { id: 'app.studio.formDesigner.importJson', defaultMessage: 'Questionnaire (FHIR JSON)', description: 'Studio: import text field' },
  fdImportVersion: { id: 'app.studio.formDesigner.importVersion', defaultMessage: 'FHIR version', description: 'Studio: import version choice' },
  fdImportR5: { id: 'app.studio.formDesigner.importR5', defaultMessage: 'R5', description: 'Studio: import as R5' },
  fdImportR4: { id: 'app.studio.formDesigner.importR4', defaultMessage: 'R4, converted to R5 with a report', description: 'Studio: import as R4' },
  fdImportRead: { id: 'app.studio.formDesigner.importRead', defaultMessage: 'Read it', description: 'Studio: read the pasted JSON' },
  fdImportCancel: { id: 'app.studio.formDesigner.importCancel', defaultMessage: 'Cancel', description: 'Studio: close the import panel' },
  fdImportReplace: { id: 'app.studio.formDesigner.importReplace', defaultMessage: 'Replace the form', description: 'Studio: replace the form with the import' },
  fdImportNotJson: { id: 'app.studio.formDesigner.importNotJson', defaultMessage: 'This is not JSON.', description: 'Studio: import refusal' },
  fdImportNotQuestionnaire: { id: 'app.studio.formDesigner.importNotQuestionnaire', defaultMessage: 'This is not a Questionnaire.', description: 'Studio: import refusal' },
  fdImportLooksR4: { id: 'app.studio.formDesigner.importLooksR4', defaultMessage: 'It uses R4’s choice types: read it as R4.', description: 'Studio: import refusal' },
  fdImportReport: { id: 'app.studio.formDesigner.importReport', defaultMessage: '{count, plural, =0 {Conversion report: nothing to convert} one {Conversion report: # line} other {Conversion report: # lines}}', description: 'Studio: conversion report heading' },
  fdImportBlocked: { id: 'app.studio.formDesigner.importBlocked', defaultMessage: 'Some of it could not be converted, so it cannot replace the form.', description: 'Studio: import blocked by the report' },
  fdImportClean: { id: 'app.studio.formDesigner.importClean', defaultMessage: 'It replaces every question of the form. The draft keeps its url, version and status.', description: 'Studio: import ready' },
  fdImportReplaced: { id: 'app.studio.formDesigner.importReplaced', defaultMessage: 'The form was replaced by the import. Save to keep it.', description: 'Studio: import done' },
  fdRChoice: { id: 'app.studio.formDesigner.rChoice', defaultMessage: '“{at}”: R4 choice became coding, options only', description: 'Studio conversion report' },
  fdROpenChoice: { id: 'app.studio.formDesigner.rOpenChoice', defaultMessage: '“{at}”: R4 open-choice became coding, options or free text', description: 'Studio conversion report' },
  fdRSource: { id: 'app.studio.formDesigner.rSource', defaultMessage: 'Its source {source} is kept in derivedFrom', description: 'Studio conversion report' },
  fdRUnknown: { id: 'app.studio.formDesigner.rUnknown', defaultMessage: '“{at}”: {element} is not converted', description: 'Studio conversion report' },
  fdRContained: { id: 'app.studio.formDesigner.rContained', defaultMessage: 'Contained resources are not converted: split SDC templates into an Extraction Mapping first', description: 'Studio conversion report' },
  fdRModifier: { id: 'app.studio.formDesigner.rModifier', defaultMessage: '“{at}”: a modifier extension is not converted', description: 'Studio conversion report' },
  fdRType: { id: 'app.studio.formDesigner.rType', defaultMessage: '“{at}”: the type {type} is not converted', description: 'Studio conversion report' },
  fdPreview: { id: 'app.studio.formDesigner.preview', defaultMessage: 'Live preview: what a health worker sees', description: 'Studio: preview heading' },
  fdProblems: { id: 'app.studio.formDesigner.problems', defaultMessage: '{count, plural, =0 {No problems found while editing} one {# problem} other {# problems}}', description: 'Studio: problems heading' },
  fdProblemsHint: { id: 'app.studio.formDesigner.problemsHint', defaultMessage: 'Check now on the package page remains the authority.', description: 'Studio: problems are a preview of the checks' },
  fdPNoId: { id: 'app.studio.formDesigner.pNoId', defaultMessage: 'A question has no id', description: 'Studio problem' },
  fdPDuplicate: { id: 'app.studio.formDesigner.pDuplicate', defaultMessage: 'Two questions share the id “{id}”', description: 'Studio problem' },
  fdPNoText: { id: 'app.studio.formDesigner.pNoText', defaultMessage: '“{id}” has no text', description: 'Studio problem' },
  fdPNoOptions: { id: 'app.studio.formDesigner.pNoOptions', defaultMessage: '“{text}” is a choice with no options', description: 'Studio problem' },
  fdPCondition: { id: 'app.studio.formDesigner.pCondition', defaultMessage: '“{text}” is shown only when a question that is not yes/no is Yes', description: 'Studio problem' },
  fdPWidget: { id: 'app.studio.formDesigner.pWidget', defaultMessage: '“{text}” cannot be shown as {widget}', description: 'Studio problem' },
  fdPNoSection: { id: 'app.studio.formDesigner.pNoSection', defaultMessage: '“{text}” is in no section: it shows at the end of the form', description: 'Studio problem' },
  fdPRemoved: { id: 'app.studio.formDesigner.pRemoved', defaultMessage: 'The published question “{id}” was removed: answers already recorded refer to it', description: 'Studio problem' },
  fdPSectionTitle: { id: 'app.studio.formDesigner.pSectionTitle', defaultMessage: 'Section {n} has no title', description: 'Studio problem' },
  fdSave: { id: 'app.studio.formDesigner.save', defaultMessage: 'Save', description: 'Studio: save the form' },
  fdSaved: { id: 'app.studio.formDesigner.saved', defaultMessage: 'Saved.', description: 'Studio: form saved' },
  fdUnsaved: { id: 'app.studio.formDesigner.unsaved', defaultMessage: 'Unsaved changes', description: 'Studio: the form has unsaved changes' },
  fdLayoutFile: { id: 'app.studio.formDesigner.layoutFile', defaultMessage: 'Its layout is {path}.', description: 'Studio: the Form Layout file' },
  fdLayoutNew: { id: 'app.studio.formDesigner.layoutNew', defaultMessage: 'It has no layout yet: a section, widget or hint adds {path}.', description: 'Studio: no Form Layout yet' },
})

type TerminologyValues = {
  tbOpen: { path: string }
  tbOpenShort: {}
  tbHeading: { title: string }
  tbBack: {}
  tbLoading: {}
  tbFailed: { reason: string }
  tbNotTerminology: {}
  tbSystems: {}
  tbSystemsNone: {}
  tbSystemRow: { title: string; version: string; count: number }
  tbLocal: {}
  tbExternal: {}
  tbOwn: {}
  tbConcepts: { title: string }
  tbPickSystem: {}
  tbSearch: {}
  tbSearchHint: {}
  tbSearchButton: {}
  tbClearSearch: {}
  tbFound: { total: number; shown: number }
  tbNoneFound: {}
  tbMore: {}
  tbShowUnder: { code: string; count: number }
  tbHideUnder: { code: string }
  tbIncluded: {}
  tbInactive: {}
  tbSelected: {}
  tbConceptCode: { code: string; system: string }
  tbDefinition: {}
  tbParents: { parents: string }
  tbTopLevel: {}
  tbContainedIn: {}
  tbContainedInNone: {}
  tbAdd: { code: string }
  tbAddUnder: { code: string }
  tbExclude: { code: string }
  tbValueSet: {}
  tbNoValueSet: {}
  tbOpenValueSet: {}
  tbNewValueSet: {}
  tbNewName: {}
  tbNewNameHint: {}
  tbNewTitle: {}
  tbCreate: {}
  tbReadOnly: {}
  tbRules: {}
  tbRulesNone: {}
  tbRuleCodes: { system: string }
  tbRuleUnder: { code: string; system: string }
  tbRuleExcluded: { system: string }
  tbRuleOther: { system: string }
  tbRemoveRule: {}
  tbRemoveCode: { code: string }
  tbExpansion: { total: number }
  tbExpansionShown: { shown: number }
  tbExpansionFailed: { reason: string }
  tbChange: {}
  tbChangeFirst: {}
  tbChangeAgainst: { published: string }
  tbChangeNone: {}
  tbAdded: { total: number }
  tbRemoved: { total: number }
  tbRemovedFlag: {}
  tbBoundBy: {}
  tbBoundByNone: {}
  tbBinding: { form: string; linkId: string; package: string }
  tbSave: {}
  tbSaved: {}
  tbUnsaved: {}
  tbStale: {}
  tbReload: {}
  tbSaveFailed: { reason: string }
}

// The Studio's terminology browser (features/studio/terminology, #523).
export const terminologyMessages = defineMessages<TerminologyValues>({
  tbOpen: { id: 'app.studio.terminology.open', defaultMessage: '{path} in the terminology browser', description: 'Studio: link from a CodeSystem or ValueSet to the terminology browser' },
  tbOpenShort: { id: 'app.studio.terminology.openShort', defaultMessage: 'Terminology', description: 'Studio: visible text of the link to the terminology browser' },
  tbHeading: { id: 'app.studio.terminology.heading', defaultMessage: 'Terminology: {title}', description: 'Studio: terminology browser heading' },
  tbBack: { id: 'app.studio.terminology.back', defaultMessage: 'Back to the package', description: 'Studio: link back to the package page' },
  tbLoading: { id: 'app.studio.terminology.loading', defaultMessage: 'Loading the terminology…', description: 'Studio: terminology loading' },
  tbFailed: { id: 'app.studio.terminology.failed', defaultMessage: 'The terminology could not be loaded: {reason}', description: 'Studio: terminology failed to load' },
  tbNotTerminology: { id: 'app.studio.terminology.notTerminology', defaultMessage: 'This file is not a CodeSystem or a ValueSet. Open it as JSON instead.', description: 'Studio: terminology browser opened on another file' },
  tbSystems: { id: 'app.studio.terminology.systems', defaultMessage: 'Code systems', description: 'Studio: the code systems column' },
  tbSystemsNone: { id: 'app.studio.terminology.systemsNone', defaultMessage: 'No code systems: this package and the published ones hold none.', description: 'Studio: no code systems' },
  tbSystemRow: { id: 'app.studio.terminology.systemRow', defaultMessage: '{title} {version} · {count, plural, one {# concept} other {# concepts}}', description: 'Studio: one code system' },
  tbLocal: { id: 'app.studio.terminology.local', defaultMessage: 'Local', description: 'Studio: a code system authored here' },
  tbExternal: { id: 'app.studio.terminology.external', defaultMessage: 'External, read only', description: 'Studio: a code system shipped in a terminology package' },
  tbOwn: { id: 'app.studio.terminology.own', defaultMessage: 'In this package', description: 'Studio: a code system of the package being edited' },
  tbConcepts: { id: 'app.studio.terminology.concepts', defaultMessage: 'Concepts of {title}', description: 'Studio: the concepts column' },
  tbPickSystem: { id: 'app.studio.terminology.pickSystem', defaultMessage: 'Choose a code system to browse.', description: 'Studio: no code system chosen' },
  tbSearch: { id: 'app.studio.terminology.search', defaultMessage: 'Search by code or words', description: 'Studio: search field label' },
  tbSearchHint: { id: 'app.studio.terminology.searchHint', defaultMessage: 'The search runs on the server, a page at a time.', description: 'Studio: search field hint' },
  tbSearchButton: { id: 'app.studio.terminology.searchButton', defaultMessage: 'Search', description: 'Studio: run the search' },
  tbClearSearch: { id: 'app.studio.terminology.clearSearch', defaultMessage: 'Back to the hierarchy', description: 'Studio: leave the search' },
  tbFound: { id: 'app.studio.terminology.found', defaultMessage: '{total, plural, one {# match} other {# matches}}, {shown} shown', description: 'Studio: search results count' },
  tbNoneFound: { id: 'app.studio.terminology.noneFound', defaultMessage: 'Nothing matches.', description: 'Studio: no search results' },
  tbMore: { id: 'app.studio.terminology.more', defaultMessage: 'Show more', description: 'Studio: load the next page of concepts' },
  tbShowUnder: { id: 'app.studio.terminology.showUnder', defaultMessage: 'Show the {count} under {code}', description: 'Studio: expand a concept in the hierarchy' },
  tbHideUnder: { id: 'app.studio.terminology.hideUnder', defaultMessage: 'Hide what is under {code}', description: 'Studio: collapse a concept in the hierarchy' },
  tbIncluded: { id: 'app.studio.terminology.included', defaultMessage: 'in the ValueSet', description: 'Studio: the concept is in the open ValueSet' },
  tbInactive: { id: 'app.studio.terminology.inactive', defaultMessage: 'retired', description: 'Studio: the concept is inactive' },
  tbSelected: { id: 'app.studio.terminology.selected', defaultMessage: 'Selected concept', description: 'Studio: the selected concept panel' },
  tbConceptCode: { id: 'app.studio.terminology.conceptCode', defaultMessage: '{code} in {system}', description: 'Studio: the selected concept code' },
  tbDefinition: { id: 'app.studio.terminology.definition', defaultMessage: 'Definition', description: 'Studio: concept definition label' },
  tbParents: { id: 'app.studio.terminology.parents', defaultMessage: 'Under {parents}', description: 'Studio: the concepts directly above' },
  tbTopLevel: { id: 'app.studio.terminology.topLevel', defaultMessage: 'A top-level concept', description: 'Studio: the concept has no parent' },
  tbContainedIn: { id: 'app.studio.terminology.containedIn', defaultMessage: 'ValueSets that contain it', description: 'Studio: ValueSets holding the concept' },
  tbContainedInNone: { id: 'app.studio.terminology.containedInNone', defaultMessage: 'No ValueSet contains it.', description: 'Studio: no ValueSet holds the concept' },
  tbAdd: { id: 'app.studio.terminology.add', defaultMessage: 'Add {code}', description: 'Studio: add one concept to the ValueSet' },
  tbAddUnder: { id: 'app.studio.terminology.addUnder', defaultMessage: 'Add {code} and everything under it', description: 'Studio: add a concept with an is-a filter' },
  tbExclude: { id: 'app.studio.terminology.exclude', defaultMessage: 'Exclude {code}', description: 'Studio: take a concept out of the ValueSet' },
  tbValueSet: { id: 'app.studio.terminology.valueSet', defaultMessage: 'ValueSet', description: 'Studio: the ValueSet column' },
  tbNoValueSet: { id: 'app.studio.terminology.noValueSet', defaultMessage: 'No ValueSet is open. Open one of this package, or start one.', description: 'Studio: no ValueSet open' },
  tbOpenValueSet: { id: 'app.studio.terminology.openValueSet', defaultMessage: 'ValueSets of this package', description: 'Studio: the package ValueSets list' },
  tbNewValueSet: { id: 'app.studio.terminology.newValueSet', defaultMessage: 'New ValueSet', description: 'Studio: start a ValueSet' },
  tbNewName: { id: 'app.studio.terminology.newName', defaultMessage: 'Id', description: 'Studio: the new ValueSet id' },
  tbNewNameHint: { id: 'app.studio.terminology.newNameHint', defaultMessage: 'Letters, digits and hyphens, e.g. danger-signs.', description: 'Studio: the new ValueSet id hint' },
  tbNewTitle: { id: 'app.studio.terminology.newTitle', defaultMessage: 'Title', description: 'Studio: the new ValueSet title' },
  tbCreate: { id: 'app.studio.terminology.create', defaultMessage: 'Start the ValueSet', description: 'Studio: create the ValueSet' },
  tbReadOnly: { id: 'app.studio.terminology.readOnly', defaultMessage: 'Read only: you may not change this package version.', description: 'Studio: the ValueSet cannot be edited' },
  tbRules: { id: 'app.studio.terminology.rules', defaultMessage: 'What it holds', description: 'Studio: the compose rules' },
  tbRulesNone: { id: 'app.studio.terminology.rulesNone', defaultMessage: 'Nothing yet: add concepts from the code system.', description: 'Studio: no rules' },
  tbRuleCodes: { id: 'app.studio.terminology.ruleCodes', defaultMessage: 'From {system}:', description: 'Studio: listed codes rule' },
  tbRuleUnder: { id: 'app.studio.terminology.ruleUnder', defaultMessage: '{code} and everything under it, from {system}', description: 'Studio: an is-a rule' },
  tbRuleExcluded: { id: 'app.studio.terminology.ruleExcluded', defaultMessage: 'Excluded from {system}:', description: 'Studio: an exclude rule' },
  tbRuleOther: { id: 'app.studio.terminology.ruleOther', defaultMessage: 'A rule on {system} this browser does not edit (kept as it is)', description: 'Studio: a rule with other filters' },
  tbRemoveRule: { id: 'app.studio.terminology.removeRule', defaultMessage: 'Remove this rule', description: 'Studio: remove a rule' },
  tbRemoveCode: { id: 'app.studio.terminology.removeCode', defaultMessage: 'Remove {code}', description: 'Studio: remove a listed code' },
  tbExpansion: { id: 'app.studio.terminology.expansion', defaultMessage: 'Expansion: {total, plural, one {# code} other {# codes}}', description: 'Studio: the expansion heading' },
  tbExpansionShown: { id: 'app.studio.terminology.expansionShown', defaultMessage: 'The first {shown} are shown.', description: 'Studio: the expansion is cut' },
  tbExpansionFailed: { id: 'app.studio.terminology.expansionFailed', defaultMessage: 'It cannot be expanded: {reason}', description: 'Studio: expansion failed' },
  tbChange: { id: 'app.studio.terminology.change', defaultMessage: 'Change against published', description: 'Studio: the change heading' },
  tbChangeFirst: { id: 'app.studio.terminology.changeFirst', defaultMessage: 'Never published: every code is new.', description: 'Studio: no published expansion' },
  tbChangeAgainst: { id: 'app.studio.terminology.changeAgainst', defaultMessage: 'Against {published}.', description: 'Studio: the published ValueSet compared' },
  tbChangeNone: { id: 'app.studio.terminology.changeNone', defaultMessage: 'The same codes as published.', description: 'Studio: no change' },
  tbAdded: { id: 'app.studio.terminology.added', defaultMessage: 'Added: {total}', description: 'Studio: codes added' },
  tbRemoved: { id: 'app.studio.terminology.removed', defaultMessage: 'Removed: {total}', description: 'Studio: codes removed' },
  tbRemovedFlag: {
    id: 'app.studio.terminology.removedFlag',
    defaultMessage: 'Removed codes stay valid in records made with the published version; new forms no longer offer them.',
    description: 'Studio: what removing a code means',
  },
  tbBoundBy: { id: 'app.studio.terminology.boundBy', defaultMessage: 'Forms that bind it', description: 'Studio: the bindings heading' },
  tbBoundByNone: { id: 'app.studio.terminology.boundByNone', defaultMessage: 'No form binds it.', description: 'Studio: no bindings' },
  tbBinding: { id: 'app.studio.terminology.binding', defaultMessage: '{form}, question {linkId} ({package})', description: 'Studio: one binding' },
  tbSave: { id: 'app.studio.terminology.save', defaultMessage: 'Save the ValueSet', description: 'Studio: save the ValueSet' },
  tbSaved: { id: 'app.studio.terminology.saved', defaultMessage: 'Saved.', description: 'Studio: ValueSet saved' },
  tbUnsaved: { id: 'app.studio.terminology.unsaved', defaultMessage: 'Unsaved changes', description: 'Studio: the ValueSet has unsaved changes' },
  tbStale: { id: 'app.studio.terminology.stale', defaultMessage: 'Someone saved this draft since you opened it, so nothing was saved. Reload to see their change.', description: 'Studio: 412 on save' },
  tbReload: { id: 'app.studio.terminology.reload', defaultMessage: 'Reload', description: 'Studio: reload after a refused save' },
  tbSaveFailed: { id: 'app.studio.terminology.saveFailed', defaultMessage: 'The ValueSet was not saved: {reason}', description: 'Studio: save failed' },
})

type WorkflowValues = {
  wsOpen: { path: string }
  wsOpenShort: {}
  wsHeading: { title: string }
  wsLoading: {}
  wsRefused: { refusal: string }
  wsFile: { path: string; url: string; version: string }
  wsProblems: { count: number }
  wsProblemsHint: {}
  wsPNoId: {}
  wsPDuplicate: { id: string }
  wsPChildren: { id: string }
  wsPInitial: { id: string }
  wsPTarget: { source: string; target: string }
  wsPStart: { target: string }
  wsPUnreachable: { id: string }
  wsPDeadEnd: { id: string }
  wsPTask: { id: string }
  wsPGate: { id: string }
  wsPAssignment: { field: string }
  wsPTable: { id: string }
  wsStates: {}
  wsAddState: {}
  wsAddInside: { id: string }
  wsExit: { target: string; on: string }
  wsEventless: {}
  wsAfter: { after: string }
  wsCanvasHeading: {}
  wsCanvas: {}
  wsCanvasHint: {}
  wsNodeTask: { role: string; worklist: string }
  wsNodeGate: {}
  wsNodeInvoke: {}
  wsNodeParallel: {}
  wsNodeCompound: {}
  wsNodeFinal: {}
  wsNodeLabel: { id: string; kind: string }
  wsEdgeLabel: { source: string; target: string; on: string }
  wsInspect: {}
  wsChoose: {}
  wsStateHeading: { id: string }
  wsId: {}
  wsIdFixed: {}
  wsIdUsed: {}
  wsIdEmpty: {}
  wsKind: {}
  wsKindName: { kind: string }
  wsParent: {}
  wsTopLevel: {}
  wsInitial: {}
  wsActivity: {}
  wsActivityName: { activity: string }
  wsRole: {}
  wsWorklist: {}
  wsForm: {}
  wsPriority: {}
  wsClaimTimeout: {}
  wsGate: {}
  wsGateHint: {}
  wsInvoke: {}
  wsEntry: {}
  wsExits: {}
  wsNone: {}
  wsAddTransition: {}
  wsTarget: {}
  wsRemoveState: { id: string }
  wsTransitionHeading: { source: string; target: string }
  wsSource: {}
  wsTrigger: {}
  wsTriggerName: { trigger: string }
  wsEvents: {}
  wsDuration: {}
  wsTimerHint: {}
  wsGuard: {}
  wsGuardName: { guard: string }
  wsExpression: {}
  wsMayRead: {}
  wsMayReadNone: {}
  wsCompetitors: {}
  wsCompetitor: { n: number; target: string; guard: string }
  wsThis: { n: number }
  wsEarlier: {}
  wsLater: {}
  wsActions: {}
  wsRemoveTransition: {}
  wsActionKind: { kind: string }
  wsActionEvent: { n: number }
  wsActionField: { n: number }
  wsActionValue: { n: number }
  wsActionTyped: { n: number; action: string; params: string }
  wsTypedLater: {}
  wsAddRaise: {}
  wsAddAssign: {}
  wsRemoveAction: { n: number }
}

// The Workflow Studio (features/studio/workflows, #498).
export const workflowMessages = defineMessages<WorkflowValues>({
  wsOpen: { id: 'app.studio.workflow.open', defaultMessage: '{path} in the Workflow Studio', description: 'Workflow Studio: link from a Workflow Definition to the Workflow Studio' },
  wsOpenShort: { id: 'app.studio.workflow.openShort', defaultMessage: 'Workflow Studio', description: 'Workflow Studio: visible text of the link to the Workflow Studio' },
  wsHeading: { id: 'app.studio.workflow.heading', defaultMessage: 'Workflow Studio: {title}', description: 'Workflow Studio: heading' },
  wsLoading: { id: 'app.studio.workflow.loading', defaultMessage: 'Loading the workflow…', description: 'Workflow Studio: loading' },
  wsRefused: { id: 'app.studio.workflow.refused', defaultMessage: '{refusal, select, not-json {This file is not JSON. Open it as JSON instead.} not-workflow {This file is not a Workflow Definition. Open it as JSON instead.} not-inline {Its definition is not inline application/vnd.health.workflow+json content. Open it as JSON instead.} other {Its definition could not be read. Open it as JSON instead.}}', description: 'Workflow Studio: the file cannot be opened' },
  wsFile: { id: 'app.studio.workflow.file', defaultMessage: '{path} · {url}|{version}', description: 'Workflow Studio: the file and the canonical of the definition' },
  wsProblems: { id: 'app.studio.workflow.problems', defaultMessage: '{count, plural, =0 {No publication problems found while editing} one {# publication problem} other {# publication problems}}', description: 'Workflow Studio: publication checks heading' },
  wsProblemsHint: { id: 'app.studio.workflow.problemsHint', defaultMessage: 'Rerun on every edit; each problem selects what it is about. Check now on the package page remains the authority: it also checks the typed actions and runs the scenarios.', description: 'Workflow Studio: publication checks hint' },
  wsPNoId: { id: 'app.studio.workflow.pNoId', defaultMessage: 'A state has no id', description: 'Workflow Studio: problem' },
  wsPDuplicate: { id: 'app.studio.workflow.pDuplicate', defaultMessage: 'Two states share the id “{id}”', description: 'Workflow Studio: problem' },
  wsPChildren: { id: 'app.studio.workflow.pChildren', defaultMessage: '“{id}” is a compound or parallel state with no state inside, or holds states its kind cannot', description: 'Workflow Studio: problem' },
  wsPInitial: { id: 'app.studio.workflow.pInitial', defaultMessage: 'The state “{id}” starts in is not inside it', description: 'Workflow Studio: problem' },
  wsPTarget: { id: 'app.studio.workflow.pTarget', defaultMessage: 'A transition from “{source}” goes to “{target}”, which is not a state', description: 'Workflow Studio: problem' },
  wsPStart: { id: 'app.studio.workflow.pStart', defaultMessage: 'The workflow starts in “{target}”, which is not a state', description: 'Workflow Studio: problem' },
  wsPUnreachable: { id: 'app.studio.workflow.pUnreachable', defaultMessage: '“{id}” cannot be reached', description: 'Workflow Studio: problem' },
  wsPDeadEnd: { id: 'app.studio.workflow.pDeadEnd', defaultMessage: '“{id}” has no way out', description: 'Workflow Studio: problem' },
  wsPTask: { id: 'app.studio.workflow.pTask', defaultMessage: 'The human task of “{id}” needs an atomic state, a role, a worklist and a form as url|version', description: 'Workflow Studio: problem' },
  wsPGate: { id: 'app.studio.workflow.pGate', defaultMessage: 'The gate “{id}” must be atomic, name its gate, have the cashier’s task and move on at gate.pass and gate.defer', description: 'Workflow Studio: problem' },
  wsPAssignment: { id: 'app.studio.workflow.pAssignment', defaultMessage: 'An action sets “{field}”, which the data model does not declare with that kind', description: 'Workflow Studio: problem' },
  wsPTable: { id: 'app.studio.workflow.pTable', defaultMessage: 'The decision table “{id}” is not well formed', description: 'Workflow Studio: problem' },
  wsStates: { id: 'app.studio.workflow.states', defaultMessage: 'States', description: 'Workflow Studio: outline heading' },
  wsAddState: { id: 'app.studio.workflow.addState', defaultMessage: 'Add a state', description: 'Workflow Studio: add a top-level state' },
  wsAddInside: { id: 'app.studio.workflow.addInside', defaultMessage: 'Add a state inside {id}', description: 'Workflow Studio: add a state inside a group' },
  wsExit: { id: 'app.studio.workflow.exit', defaultMessage: '→ {target} on {on}', description: 'Workflow Studio: an exit in the outline' },
  wsEventless: { id: 'app.studio.workflow.eventless', defaultMessage: 'no event', description: 'Workflow Studio: an eventless transition' },
  wsAfter: { id: 'app.studio.workflow.after', defaultMessage: 'timer after {after}', description: 'Workflow Studio: a timer transition' },
  wsCanvasHeading: { id: 'app.studio.workflow.canvasHeading', defaultMessage: 'Canvas', description: 'Workflow Studio: canvas heading' },
  wsCanvas: { id: 'app.studio.workflow.canvas', defaultMessage: 'Statechart', description: 'Workflow Studio: canvas accessible name' },
  wsCanvasHint: { id: 'app.studio.workflow.canvasHint', defaultMessage: 'Drag from a state’s right edge to another state to add a transition, or add one from the inspector.', description: 'Workflow Studio: canvas hint' },
  wsNodeTask: { id: 'app.studio.workflow.nodeTask', defaultMessage: 'Task · {role} · {worklist}', description: 'Workflow Studio: canvas node detail' },
  wsNodeGate: { id: 'app.studio.workflow.nodeGate', defaultMessage: 'Payment gate', description: 'Workflow Studio: canvas node detail' },
  wsNodeInvoke: { id: 'app.studio.workflow.nodeInvoke', defaultMessage: 'Sub-workflow', description: 'Workflow Studio: canvas node detail' },
  wsNodeParallel: { id: 'app.studio.workflow.nodeParallel', defaultMessage: 'parallel: every region at once', description: 'Workflow Studio: canvas node detail' },
  wsNodeCompound: { id: 'app.studio.workflow.nodeCompound', defaultMessage: 'compound', description: 'Workflow Studio: canvas node detail' },
  wsNodeFinal: { id: 'app.studio.workflow.nodeFinal', defaultMessage: 'final', description: 'Workflow Studio: canvas node detail' },
  wsNodeLabel: { id: 'app.studio.workflow.nodeLabel', defaultMessage: '{kind} state {id}', description: 'Workflow Studio: canvas node accessible name' },
  wsEdgeLabel: { id: 'app.studio.workflow.edgeLabel', defaultMessage: 'Transition from {source} to {target} on {on}', description: 'Workflow Studio: canvas edge accessible name' },
  wsInspect: { id: 'app.studio.workflow.inspect', defaultMessage: 'Inspect', description: 'Workflow Studio: inspector heading' },
  wsChoose: { id: 'app.studio.workflow.choose', defaultMessage: 'Select a state or a transition in the outline or on the canvas.', description: 'Workflow Studio: nothing selected' },
  wsStateHeading: { id: 'app.studio.workflow.stateHeading', defaultMessage: 'State {id}', description: 'Workflow Studio: inspector: a state' },
  wsId: { id: 'app.studio.workflow.id', defaultMessage: 'Id', description: 'Workflow Studio: field' },
  wsIdFixed: { id: 'app.studio.workflow.idFixed', defaultMessage: 'Fixed: the published version has this state, and running instances and migration maps refer to it.', description: 'Workflow Studio: hint' },
  wsIdUsed: { id: 'app.studio.workflow.idUsed', defaultMessage: 'Another state has this id.', description: 'Workflow Studio: error' },
  wsIdEmpty: { id: 'app.studio.workflow.idEmpty', defaultMessage: 'A state needs an id.', description: 'Workflow Studio: error' },
  wsKind: { id: 'app.studio.workflow.kind', defaultMessage: 'Kind', description: 'Workflow Studio: field' },
  wsKindName: { id: 'app.studio.workflow.kindName', defaultMessage: '{kind, select, atomic {Atomic} compound {Compound: one state inside at a time} parallel {Parallel: every state inside at once} final {Final} other {{kind}}}', description: 'Workflow Studio: a state kind' },
  wsParent: { id: 'app.studio.workflow.parent', defaultMessage: 'Inside', description: 'Workflow Studio: field' },
  wsTopLevel: { id: 'app.studio.workflow.topLevel', defaultMessage: 'The workflow itself (top level)', description: 'Workflow Studio: option' },
  wsInitial: { id: 'app.studio.workflow.initial', defaultMessage: 'Starts in', description: 'Workflow Studio: field' },
  wsActivity: { id: 'app.studio.workflow.activity', defaultMessage: 'What happens there', description: 'Workflow Studio: field' },
  wsActivityName: { id: 'app.studio.workflow.activityName', defaultMessage: '{activity, select, wait {Wait for an event} task {A human task} gate {A payment gate} invoke {A sub-workflow} other {{activity}}}', description: 'Workflow Studio: an activity' },
  wsRole: { id: 'app.studio.workflow.role', defaultMessage: 'Role', description: 'Workflow Studio: field' },
  wsWorklist: { id: 'app.studio.workflow.worklist', defaultMessage: 'Worklist', description: 'Workflow Studio: field' },
  wsForm: { id: 'app.studio.workflow.form', defaultMessage: 'Form (Questionnaire url|version)', description: 'Workflow Studio: field' },
  wsPriority: { id: 'app.studio.workflow.priority', defaultMessage: 'Priority', description: 'Workflow Studio: field' },
  wsClaimTimeout: { id: 'app.studio.workflow.claimTimeout', defaultMessage: 'Claim timeout (ISO 8601 duration, e.g. PT15M)', description: 'Workflow Studio: field' },
  wsGate: { id: 'app.studio.workflow.gate', defaultMessage: 'Financial Gate configuration (canonical URL)', description: 'Workflow Studio: field' },
  wsGateHint: { id: 'app.studio.workflow.gateHint', defaultMessage: 'Finance answers gate.pass, gate.defer or gate.block; while it blocks, the task is the cashier’s.', description: 'Workflow Studio: hint' },
  wsInvoke: { id: 'app.studio.workflow.invoke', defaultMessage: 'Sub-workflow (url|version)', description: 'Workflow Studio: field' },
  wsEntry: { id: 'app.studio.workflow.entry', defaultMessage: 'Entry actions', description: 'Workflow Studio: legend' },
  wsExits: { id: 'app.studio.workflow.exits', defaultMessage: 'Transitions out, in document order', description: 'Workflow Studio: legend' },
  wsNone: { id: 'app.studio.workflow.none', defaultMessage: 'None.', description: 'Workflow Studio: empty list' },
  wsAddTransition: { id: 'app.studio.workflow.addTransition', defaultMessage: 'Add a transition', description: 'Workflow Studio: button' },
  wsTarget: { id: 'app.studio.workflow.target', defaultMessage: 'To', description: 'Workflow Studio: field' },
  wsRemoveState: { id: 'app.studio.workflow.removeState', defaultMessage: 'Remove state {id}', description: 'Workflow Studio: button' },
  wsTransitionHeading: { id: 'app.studio.workflow.transitionHeading', defaultMessage: 'Transition from {source} to {target}', description: 'Workflow Studio: inspector: a transition' },
  wsSource: { id: 'app.studio.workflow.source', defaultMessage: 'From', description: 'Workflow Studio: field' },
  wsTrigger: { id: 'app.studio.workflow.trigger', defaultMessage: 'Triggered by', description: 'Workflow Studio: field' },
  wsTriggerName: { id: 'app.studio.workflow.triggerName', defaultMessage: '{trigger, select, event {An event} timer {A timer} eventless {Nothing (eventless)} other {{trigger}}}', description: 'Workflow Studio: a trigger' },
  wsEvents: { id: 'app.studio.workflow.events', defaultMessage: 'Events (comma separated)', description: 'Workflow Studio: field' },
  wsDuration: { id: 'app.studio.workflow.duration', defaultMessage: 'After (ISO 8601 duration, e.g. PT1H)', description: 'Workflow Studio: field' },
  wsTimerHint: { id: 'app.studio.workflow.timerHint', defaultMessage: 'A timer runs as the workflow principal.', description: 'Workflow Studio: hint' },
  wsGuard: { id: 'app.studio.workflow.guard', defaultMessage: 'Guard', description: 'Workflow Studio: field' },
  wsGuardName: { id: 'app.studio.workflow.guardName', defaultMessage: '{guard, select, always {Always} fhirpath {A FHIRPath expression} other {{guard}}}', description: 'Workflow Studio: a guard kind' },
  wsExpression: { id: 'app.studio.workflow.expression', defaultMessage: 'FHIRPath expression', description: 'Workflow Studio: field' },
  wsMayRead: { id: 'app.studio.workflow.mayRead', defaultMessage: 'It may read', description: 'Workflow Studio: the declared data a guard may read' },
  wsMayReadNone: { id: 'app.studio.workflow.mayReadNone', defaultMessage: 'Nothing is declared yet.', description: 'Workflow Studio: no declared data' },
  wsCompetitors: { id: 'app.studio.workflow.competitors', defaultMessage: 'Other transitions on the same event, in document order: the first whose guard holds is taken', description: 'Workflow Studio: legend' },
  wsCompetitor: { id: 'app.studio.workflow.competitor', defaultMessage: '{n}. → {target} {guard}', description: 'Workflow Studio: a competing transition' },
  wsThis: { id: 'app.studio.workflow.this', defaultMessage: '{n}. this transition', description: 'Workflow Studio: this transition among its competitors' },
  wsEarlier: { id: 'app.studio.workflow.earlier', defaultMessage: 'Move earlier', description: 'Workflow Studio: button' },
  wsLater: { id: 'app.studio.workflow.later', defaultMessage: 'Move later', description: 'Workflow Studio: button' },
  wsActions: { id: 'app.studio.workflow.actions', defaultMessage: 'Actions', description: 'Workflow Studio: legend' },
  wsRemoveTransition: { id: 'app.studio.workflow.removeTransition', defaultMessage: 'Remove this transition', description: 'Workflow Studio: button' },
  wsActionKind: { id: 'app.studio.workflow.actionKind', defaultMessage: '{kind, select, raise {Raise an event} assign {Set a field} execute {Typed action} other {{kind}}}', description: 'Workflow Studio: an action kind' },
  wsActionEvent: { id: 'app.studio.workflow.actionEvent', defaultMessage: 'Event of action {n}', description: 'Workflow Studio: field' },
  wsActionField: { id: 'app.studio.workflow.actionField', defaultMessage: 'Field of action {n}', description: 'Workflow Studio: field' },
  wsActionValue: { id: 'app.studio.workflow.actionValue', defaultMessage: 'Value of action {n}', description: 'Workflow Studio: field' },
  wsActionTyped: { id: 'app.studio.workflow.actionTyped', defaultMessage: 'Action {n}: {action} {params}', description: 'Workflow Studio: a typed action, read only' },
  wsTypedLater: { id: 'app.studio.workflow.typedLater', defaultMessage: 'Typed actions are kept as read here; picking them from the registered list comes next.', description: 'Workflow Studio: hint' },
  wsAddRaise: { id: 'app.studio.workflow.addRaise', defaultMessage: 'Add: raise an event', description: 'Workflow Studio: button' },
  wsAddAssign: { id: 'app.studio.workflow.addAssign', defaultMessage: 'Add: set a field', description: 'Workflow Studio: button' },
  wsRemoveAction: { id: 'app.studio.workflow.removeAction', defaultMessage: 'Remove action {n}', description: 'Workflow Studio: button' },
})
