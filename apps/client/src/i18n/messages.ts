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

type AreaValues = {
  arTitle: {}
  arLoading: {}
  arFailed: {}
  arNotHere: {}
  arPlace: { name: string; level: string }
  arAreaIntro: { count: number }
  arCode: {}
  arName: {}
  arCareLevel: {}
  arStatus: {}
  arOpen: {}
  arClosed: {}
  arLevel_facility: {}
  arLevel_district: {}
  arLevel_region: {}
  arLevel_national: {}
}

// The area page's strings (features/area; ADR 0094).
export const areaMessages = defineMessages<AreaValues>({
  arTitle: { id: 'app.area.title', defaultMessage: 'My area', description: 'Area page heading and home link' },
  arLoading: { id: 'app.area.loading', defaultMessage: 'Loading…', description: 'Area page: loading' },
  arFailed: { id: 'app.area.failed', defaultMessage: 'Your area could not be loaded. Please try again.', description: 'Area page: loading failed' },
  arNotHere: { id: 'app.area.notHere', defaultMessage: 'Areas are kept at central.', description: 'Area page: shown away from central' },
  arPlace: { id: 'app.area.place', defaultMessage: 'You are signed in for {name} ({level}).', description: 'Area page: the place the session acts for and its level' },
  arAreaIntro: {
    id: 'app.area.intro',
    defaultMessage: '{count, plural, one {# facility is} other {# facilities are}} in your area. You see their summaries and reports, not patient records.',
    description: 'Area page: how many facilities the area holds',
  },
  arCode: { id: 'app.area.code', defaultMessage: 'Code', description: 'Area page: facility code column' },
  arName: { id: 'app.area.name', defaultMessage: 'Facility', description: 'Area page: facility name column' },
  arCareLevel: { id: 'app.area.careLevel', defaultMessage: 'Level of care', description: 'Area page: the NHFR level of care column' },
  arStatus: { id: 'app.area.status', defaultMessage: 'Status', description: 'Area page: open or closed column' },
  arOpen: { id: 'app.area.open', defaultMessage: 'Open', description: 'Area page: the facility is open' },
  arClosed: { id: 'app.area.closed', defaultMessage: 'Closed', description: 'Area page: the NHFR lists the facility as closed' },
  arLevel_facility: { id: 'app.area.level.facility', defaultMessage: 'facility', description: 'Area page: place level' },
  arLevel_district: { id: 'app.area.level.district', defaultMessage: 'district', description: 'Area page: place level' },
  arLevel_region: { id: 'app.area.level.region', defaultMessage: 'region', description: 'Area page: place level' },
  arLevel_national: { id: 'app.area.level.national', defaultMessage: 'national', description: 'Area page: place level' },
})

type PlacesValues = {
  plImportTitle: {}
  plImportIntro: {}
  plFile: {}
  plPreview: {}
  plApply: {}
  plApplied: { count: number }
  plNothing: {}
  plHierarchy: { count: number }
  plNew: { count: number }
  plChanged: { count: number }
  plClosed: { count: number }
  plClosedNote: {}
  plAbsent: { count: number }
  plAbsentNote: {}
  plUnchanged: { count: number }
  plChange: { name: string; code: string; fields: string }
  plField_name: {}
  plField_parent: {}
  plField_active: {}
  plField_care_level: {}
  plField_ownership: {}
  plField_coordinates: {}
  plRefused: {}
  plImportForbidden: {}
  plFailed: { reason: string }
  plRolesTitle: {}
  plRolesIntro: {}
  plRolesForbidden: {}
  plLoading: {}
  plUsername: {}
  plFind: {}
  plNoSuchUser: {}
  plHeld: { username: string }
  plNone: {}
  plHeldLine: { place: string; roles: string; until: string }
  plWithdraw: {}
  plWithdrawConfirm: { place: string }
  plAssignHeading: {}
  plPlace: {}
  plChoosePlace: {}
  plRoles: {}
  plReplaceNote: {}
  plSave: {}
  plAssigned: { username: string; roles: string; place: string }
  plCannotGrant: { role: string; permission: string }
  plWithdrawn: { place: string }
}

// The facility registry's administration pages (features/places; ADR 0094; #873).
export const placesMessages = defineMessages<PlacesValues>({
  plImportTitle: { id: 'app.places.import.title', defaultMessage: 'Import facilities from the NHFR', description: 'NHFR import page heading and home link' },
  plImportIntro: { id: 'app.places.import.intro', defaultMessage: 'Choose the NHFR facility export (CSV). You review what it would add, change and close before anything is imported.', description: 'NHFR import page: what it does' },
  plFile: { id: 'app.places.import.file', defaultMessage: 'NHFR export (CSV)', description: 'NHFR import page: the file picker' },
  plPreview: { id: 'app.places.import.preview', defaultMessage: 'Review the import', description: 'NHFR import page: preview button' },
  plApply: { id: 'app.places.import.apply', defaultMessage: 'Import', description: 'NHFR import page: apply button' },
  plApplied: { id: 'app.places.import.applied', defaultMessage: 'Imported: {count, plural, one {# place} other {# places}} written.', description: 'NHFR import page: the import was applied' },
  plNothing: { id: 'app.places.import.nothing', defaultMessage: 'The registry already matches this file. There is nothing to import.', description: 'NHFR import page: the file changes nothing' },
  plHierarchy: { id: 'app.places.import.hierarchy', defaultMessage: 'Regions and districts to add ({count})', description: 'NHFR import page: section of regions and districts added' },
  plNew: { id: 'app.places.import.new', defaultMessage: 'New facilities ({count})', description: 'NHFR import page: section of new facilities' },
  plChanged: { id: 'app.places.import.changed', defaultMessage: 'Changed facilities ({count})', description: 'NHFR import page: section of changed facilities' },
  plClosed: { id: 'app.places.import.closed', defaultMessage: 'Closed facilities ({count})', description: 'NHFR import page: section of facilities the NHFR closed' },
  plClosedNote: { id: 'app.places.import.closedNote', defaultMessage: 'A closed facility is kept, marked closed, because its records still refer to it.', description: 'NHFR import page: closed facilities are not deleted' },
  plAbsent: { id: 'app.places.import.absent', defaultMessage: 'Not in this file ({count})', description: 'NHFR import page: section of facilities the file does not list' },
  plAbsentNote: { id: 'app.places.import.absentNote', defaultMessage: 'These facilities are left as they are: a partial export does not close them.', description: 'NHFR import page: absent facilities are left alone' },
  plUnchanged: { id: 'app.places.import.unchanged', defaultMessage: '{count, plural, one {# facility is} other {# facilities are}} unchanged.', description: 'NHFR import page: how many facilities are unchanged' },
  plChange: { id: 'app.places.import.change', defaultMessage: '{name} ({code}): {fields}', description: 'NHFR import page: a changed facility and the fields that change' },
  plField_name: { id: 'app.places.import.field.name', defaultMessage: 'name', description: 'NHFR import page: changed field' },
  plField_parent: { id: 'app.places.import.field.parent', defaultMessage: 'district', description: 'NHFR import page: changed field' },
  plField_active: { id: 'app.places.import.field.active', defaultMessage: 'status', description: 'NHFR import page: changed field' },
  plField_care_level: { id: 'app.places.import.field.careLevel', defaultMessage: 'level of care', description: 'NHFR import page: changed field' },
  plField_ownership: { id: 'app.places.import.field.ownership', defaultMessage: 'ownership', description: 'NHFR import page: changed field' },
  plField_coordinates: { id: 'app.places.import.field.coordinates', defaultMessage: 'coordinates', description: 'NHFR import page: changed field' },
  plRefused: { id: 'app.places.import.refused', defaultMessage: 'The file cannot be imported:', description: 'NHFR import page: the file was refused, followed by its problems' },
  plImportForbidden: { id: 'app.places.import.forbidden', defaultMessage: 'Your role does not include importing the NHFR.', description: 'NHFR import page: the user lacks places.manage' },
  plFailed: { id: 'app.places.failed', defaultMessage: 'The request failed: {reason}', description: 'Places pages: a request failed' },
  plRolesTitle: { id: 'app.places.roles.title', defaultMessage: 'Roles at places', description: 'Role assignment page heading and home link' },
  plRolesIntro: { id: 'app.places.roles.intro', defaultMessage: 'A role is held at a place: a facility, a district, a region or the national place. At a district, region or the national place it gives its permissions over every place below, never over patient records. You assign roles within your own area.', description: 'Role assignment page: what it does' },
  plRolesForbidden: { id: 'app.places.roles.forbidden', defaultMessage: 'Your role does not include assigning roles.', description: 'Role assignment page: the user lacks access.administer' },
  plLoading: { id: 'app.places.loading', defaultMessage: 'Loading…', description: 'Places pages: loading' },
  plUsername: { id: 'app.places.roles.username', defaultMessage: 'Username', description: 'Role assignment page: whose roles' },
  plFind: { id: 'app.places.roles.find', defaultMessage: 'Show roles', description: 'Role assignment page: look the user up' },
  plNoSuchUser: { id: 'app.places.roles.noSuchUser', defaultMessage: 'There is no user with that username.', description: 'Role assignment page: unknown username' },
  plHeld: { id: 'app.places.roles.held', defaultMessage: 'Roles {username} holds', description: "Role assignment page: the user's current roles" },
  plNone: { id: 'app.places.roles.none', defaultMessage: 'No roles at any place.', description: 'Role assignment page: the user holds no roles' },
  plHeldLine: { id: 'app.places.roles.heldLine', defaultMessage: '{place}: {roles}, until {until}', description: 'Role assignment page: roles held at a place and when they expire' },
  plWithdraw: { id: 'app.places.roles.withdraw', defaultMessage: 'Withdraw', description: 'Role assignment page: withdraw every role at the place' },
  plWithdrawConfirm: { id: 'app.places.roles.withdrawConfirm', defaultMessage: 'Confirm: withdraw every role at {place}', description: 'Role assignment page: confirm a withdrawal' },
  plAssignHeading: { id: 'app.places.roles.assign', defaultMessage: 'Assign roles at a place', description: 'Role assignment page: the assignment form' },
  plPlace: { id: 'app.places.roles.place', defaultMessage: 'Place', description: 'Role assignment page: the place picker' },
  plChoosePlace: { id: 'app.places.roles.choosePlace', defaultMessage: 'Choose a place', description: 'Role assignment page: no place chosen' },
  plRoles: { id: 'app.places.roles.roles', defaultMessage: 'Roles', description: 'Role assignment page: the roles to hold at the place' },
  plReplaceNote: { id: 'app.places.roles.replace', defaultMessage: 'The roles chosen replace those the user holds at that place.', description: 'Role assignment page: assigning replaces the roles at the place' },
  plSave: { id: 'app.places.roles.save', defaultMessage: 'Assign', description: 'Role assignment page: save button' },
  plCannotGrant: { id: 'app.places.roles.cannotGrant', defaultMessage: 'You cannot grant {role}: it carries {permission}, which your own roles do not give you.', description: 'Role assignment page: the administrator tried to grant a permission they do not hold' },
  plAssigned: { id: 'app.places.roles.assigned', defaultMessage: '{username} now holds {roles} at {place}. They sign in for it from their next sign-in.', description: 'Role assignment page: the roles were assigned' },
  plWithdrawn: { id: 'app.places.roles.withdrawn', defaultMessage: 'Every role at {place} was withdrawn.', description: 'Role assignment page: the roles at a place were withdrawn' },
})

type ExplorerValues = {
  exTitle: {}
  exLoading: {}
  exFailed: {}
  exForbidden: {}
  exPickers: {}
  exData: {}
  exNoIndicators: {}
  exPeriods: {}
  exPeriodType: {}
  exPeriodType_weekly: {}
  exPeriodType_monthly: {}
  exPeriodType_quarterly: {}
  exPeriodType_financialJuly: {}
  exFrom: {}
  exTo: {}
  exPlaces: {}
  exVisualization: {}
  exPickSomething: {}
  exChartLabel: { indicators: string; from: string; to: string }
  exFigures: {}
  exSeries: {}
  exWhereCentral: {}
  exWhereFacility: {}
  exAsOf: { at: string }
  exNeverComputed: {}
  exCompletenessTitle: {}
  exCompleteness: { period: string; place: string; reported: number; expected: number }
  exMissingTitle: {}
  exMissingLine: { period: string; place: string; facilities: string }
  exNeverSynced: { name: string }
  exLastSynced: { name: string; at: string }
  exFacilityOnly: { indicator: string }
  exOverEstimate: { indicator: string; estimate: string }
  exNoEstimate: { estimate: string; places: string }
  exChartType: {}
  exType_column: {}
  exType_line: {}
  exType_table: {}
  exType_single: {}
  exType_map: {}
  exMapLabel: { indicator: string; period: string }
  exMapNoGeometry: {}
  exMapPlace: { name: string; figure: string }
  exMapNoFigure: {}
  exMapHollow: {}
  exMapLoading: {}
  exLayout: {}
  exLayout_placesByIndicators: {}
  exLayout_indicatorsByPeriods: {}
  exPlace: {}
  exLineChartLabel: { indicators: string; from: string; to: string }
  exSingleNumbers: {}
  exPercent: { value: string }
  exChange: { change: string; period: string; previous: string }
  exNoChange: { period: string }
  exTone_good: {}
  exTone_warning: {}
  exTone_bad: {}
  exLegend: { indicator: string }
  exBand: { range: string; tone: string }
  exBandBelow: { to: string }
  exBandFrom: { from: string }
  exBandBetween: { from: string; to: string }
  exBandEvery: {}
  exDisaggregation: {}
  exNoDisaggregation: {}
  exSeriesBy: {}
  exSeriesBy_indicator: {}
  exSeriesBy_place: {}
  exSeriesBy_band: {}
  exNotRecorded: {}
  exChildAreas: { place: string }
  exBandHeader: {}
  exFollowsPlace: {}
  exFollowsAreas: {}
  exSave: {}
  exSaveTitle: {}
  exSaveOnto: {}
  exSaveOntoNone: {}
  exSaveOntoNew: {}
  exNewDashboardTitle: {}
  exSaveConfirm: {}
  exSaveCancel: {}
  exSaveFailed: {}
  exSaved: {}
  exSavedOnto: { dashboard: string }
  dbTitle: {}
  dbLoading: {}
  dbFailed: {}
  dbForbidden: {}
  dbList: {}
  dbStandard: {}
  dbNoStandard: {}
  dbSaved: {}
  dbNoSaved: {}
  dbSavedAtCentral: {}
  dbMine: {}
  dbSharedWithYou: {}
  dbUnplaced: {}
  dbPick: {}
  dbEmpty: {}
  dbOpenInExplorer: {}
  dbNew: {}
  dbNewTitle: {}
  dbCreate: {}
  dbSharing: {}
  dbSharingIntro: {}
  dbLevels: {}
  dbLevel_facility: {}
  dbLevel_district: {}
  dbLevel_region: {}
  dbLevel_national: {}
  dbRoles: {}
  dbPeople: {}
  dbListHint: {}
  dbSaveSharing: {}
  dbSharingSaved: {}
  dbChangeFailed: {}
  dbSharedWith: { shares: string }
  dbNotShared: {}
  dbStandardFrom: { package: string }
  dbStandardNote: {}
  dbSaveAs: {}
  dbSaveAsTitle: {}
  dbSaveAsCopy: { title: string }
  dbSaveAsButton: {}
}

// The data explorer's strings (features/explorer; ADR 0075).
export const explorerMessages = defineMessages<ExplorerValues>({
  exTitle: { id: 'app.explorer.title', defaultMessage: 'Data explorer', description: 'Data explorer heading and home link' },
  exLoading: { id: 'app.explorer.loading', defaultMessage: 'Loading…', description: 'Data explorer: loading' },
  exFailed: { id: 'app.explorer.failed', defaultMessage: 'The figures could not be loaded. Please try again.', description: 'Data explorer: loading failed' },
  exForbidden: { id: 'app.explorer.forbidden', defaultMessage: 'Exploring indicators needs the reporting.view permission.', description: 'Data explorer: the user may not explore' },
  exPickers: { id: 'app.explorer.pickers', defaultMessage: 'What to show', description: 'Data explorer: the pickers form' },
  exData: { id: 'app.explorer.data', defaultMessage: 'Data', description: 'Data explorer: the indicators picker' },
  exNoIndicators: { id: 'app.explorer.noIndicators', defaultMessage: 'No indicators are published yet.', description: 'Data explorer: nothing to pick' },
  exPeriods: { id: 'app.explorer.periods', defaultMessage: 'Periods', description: 'Data explorer: the periods picker' },
  exPeriodType: { id: 'app.explorer.periodType', defaultMessage: 'Period type', description: 'Data explorer: weeks, months, quarters or financial years' },
  exPeriodType_weekly: { id: 'app.explorer.periodType.weekly', defaultMessage: 'Weeks', description: 'Data explorer: period type' },
  exPeriodType_monthly: { id: 'app.explorer.periodType.monthly', defaultMessage: 'Months', description: 'Data explorer: period type' },
  exPeriodType_quarterly: { id: 'app.explorer.periodType.quarterly', defaultMessage: 'Quarters', description: 'Data explorer: period type' },
  exPeriodType_financialJuly: { id: 'app.explorer.periodType.financialJuly', defaultMessage: 'Financial years (July)', description: 'Data explorer: period type' },
  exFrom: { id: 'app.explorer.from', defaultMessage: 'From', description: 'Data explorer: the first period' },
  exTo: { id: 'app.explorer.to', defaultMessage: 'To', description: 'Data explorer: the last period' },
  exPlaces: { id: 'app.explorer.places', defaultMessage: 'Places', description: 'Data explorer: the places picker' },
  exVisualization: { id: 'app.explorer.visualization', defaultMessage: 'Visualization', description: 'Data explorer: the chart card' },
  exPickSomething: { id: 'app.explorer.pickSomething', defaultMessage: 'Pick at least one indicator and one place.', description: 'Data explorer: nothing picked' },
  exChartLabel: { id: 'app.explorer.chartLabel', defaultMessage: 'Column chart of {indicators}, {from} to {to}', description: 'Data explorer: the chart, for screen readers' },
  exFigures: { id: 'app.explorer.figures', defaultMessage: 'Figures', description: 'Data explorer: the table of the charted figures' },
  exSeries: { id: 'app.explorer.series', defaultMessage: 'Indicator', description: 'Data explorer: the figures table row header' },
  exWhereCentral: { id: 'app.explorer.whereCentral', defaultMessage: 'Rolled up at central from the facilities’ summaries.', description: 'Data explorer: where the figures were computed' },
  exWhereFacility: { id: 'app.explorer.whereFacility', defaultMessage: 'Computed on this facility.', description: 'Data explorer: where the figures were computed' },
  exAsOf: { id: 'app.explorer.asOf', defaultMessage: 'Newest summary computed {at}.', description: 'Data explorer: freshness' },
  exNeverComputed: { id: 'app.explorer.neverComputed', defaultMessage: 'No summary has been computed for these periods.', description: 'Data explorer: no figures at all' },
  exCompletenessTitle: { id: 'app.explorer.completenessTitle', defaultMessage: 'Reporting completeness', description: 'Data explorer: completeness list' },
  exCompleteness: {
    id: 'app.explorer.completeness',
    defaultMessage: '{period}, {place}: {reported} of {expected, plural, one {# facility} other {# facilities}} reported.',
    description: 'Data explorer: facilities reported out of expected for the latest period',
  },
  exMissingTitle: { id: 'app.explorer.missingTitle', defaultMessage: 'Missing, not counted as zero:', description: 'Data explorer: facilities that have not synced' },
  exMissingLine: { id: 'app.explorer.missingLine', defaultMessage: '{period}, {place}: {facilities}', description: 'Data explorer: the facilities missing for a period and place' },
  exNeverSynced: { id: 'app.explorer.neverSynced', defaultMessage: '{name} (never synced)', description: 'Data explorer: a missing facility that has never synced' },
  exLastSynced: { id: 'app.explorer.lastSynced', defaultMessage: '{name} (last synced {at})', description: 'Data explorer: a missing facility and its last sync' },
  exOverEstimate: {
    id: 'app.explorer.overEstimate',
    defaultMessage: '{indicator} is over each place’s {estimate} for the period, from the year’s Population estimates.',
    description: 'Data explorer: an indicator whose denominator is a Population Estimate type (#553)',
  },
  exNoEstimate: {
    id: 'app.explorer.noEstimate',
    defaultMessage: 'No estimate of {estimate} for {places}: no figure is shown rather than a guessed share.',
    description: 'Data explorer: places without a Population Estimate, so no coverage figure (#553)',
  },
  exFacilityOnly: {
    id: 'app.explorer.facilityOnly',
    defaultMessage: '{indicator} shows at facility level only: a median or a distinct count cannot be rebuilt from the facilities’ summaries.',
    description: 'Data explorer: why a median is absent above facility level',
  },
  exChartType: { id: 'app.explorer.chartType', defaultMessage: 'Chart type', description: 'Data explorer: the Visualization type buttons' },
  exType_column: { id: 'app.explorer.type.column', defaultMessage: 'Column', description: 'Data explorer: Visualization type' },
  exType_line: { id: 'app.explorer.type.line', defaultMessage: 'Line', description: 'Data explorer: Visualization type' },
  exType_table: { id: 'app.explorer.type.table', defaultMessage: 'Table', description: 'Data explorer: Visualization type' },
  exType_single: { id: 'app.explorer.type.single', defaultMessage: 'Single number', description: 'Data explorer: Visualization type' },
  exType_map: { id: 'app.explorer.type.map', defaultMessage: 'Map', description: 'Data explorer: Visualization type' },
  exMapLabel: { id: 'app.explorer.mapLabel', defaultMessage: 'Map of {indicator}, {period}', description: 'Data explorer: a map’s accessible name' },
  exMapNoGeometry: { id: 'app.explorer.mapNoGeometry', defaultMessage: 'None of these places has a boundary or coordinates to draw.', description: 'Data explorer: nothing to draw on a map' },
  exMapPlace: { id: 'app.explorer.mapPlace', defaultMessage: '{name}: {figure}', description: 'Data explorer: a place on a map, with its figure' },
  exMapNoFigure: { id: 'app.explorer.mapNoFigure', defaultMessage: 'no figure', description: 'Data explorer: a place on a map without a figure' },
  exMapHollow: { id: 'app.explorer.mapHollow', defaultMessage: 'A hollow point is a facility that has not synced this period.', description: 'Data explorer: the key to hollow facility points on a map' },
  exMapLoading: { id: 'app.explorer.mapLoading', defaultMessage: 'Loading the map…', description: 'Data explorer: the map’s places are loading' },
  exLayout: { id: 'app.explorer.layout', defaultMessage: 'Table layout', description: 'Data explorer: how the table lays out its figures' },
  exLayout_placesByIndicators: { id: 'app.explorer.layout.placesByIndicators', defaultMessage: 'Places × indicators', description: 'Data explorer: places down the side, indicators across' },
  exLayout_indicatorsByPeriods: { id: 'app.explorer.layout.indicatorsByPeriods', defaultMessage: 'Indicators × periods', description: 'Data explorer: indicators down the side, periods across' },
  exPlace: { id: 'app.explorer.place', defaultMessage: 'Place', description: 'Data explorer: the table row header when places are down the side' },
  exLineChartLabel: { id: 'app.explorer.lineChartLabel', defaultMessage: 'Line chart of {indicators}, {from} to {to}', description: 'Data explorer: the line chart, for screen readers' },
  exSingleNumbers: { id: 'app.explorer.singleNumbers', defaultMessage: 'Single numbers', description: 'Data explorer: the single-number tiles' },
  exPercent: { id: 'app.explorer.percent', defaultMessage: '{value}%', description: 'Data explorer: a percentage figure' },
  exChange: { id: 'app.explorer.change', defaultMessage: '{change} on {period} ({previous})', description: 'Data explorer: a single number’s change on the period before, and that period’s figure' },
  exNoChange: { id: 'app.explorer.noChange', defaultMessage: 'No figure for {period} to compare with', description: 'Data explorer: no figure for the period before' },
  exTone_good: { id: 'app.explorer.tone.good', defaultMessage: 'Good', description: 'Data explorer: a threshold band' },
  exTone_warning: { id: 'app.explorer.tone.warning', defaultMessage: 'Warning', description: 'Data explorer: a threshold band' },
  exTone_bad: { id: 'app.explorer.tone.bad', defaultMessage: 'Bad', description: 'Data explorer: a threshold band' },
  exLegend: { id: 'app.explorer.legend', defaultMessage: '{indicator} thresholds', description: 'Data explorer: an indicator’s threshold legend' },
  exBand: { id: 'app.explorer.band', defaultMessage: '{range}: {tone}', description: 'Data explorer: one threshold band in a legend' },
  exBandBelow: { id: 'app.explorer.band.below', defaultMessage: 'below {to}', description: 'Data explorer: a band with no lower bound' },
  exBandFrom: { id: 'app.explorer.band.from', defaultMessage: '{from} and above', description: 'Data explorer: a band with no upper bound' },
  exBandBetween: { id: 'app.explorer.band.between', defaultMessage: '{from} to under {to}', description: 'Data explorer: a band from one figure up to another' },
  exBandEvery: { id: 'app.explorer.band.every', defaultMessage: 'every figure', description: 'Data explorer: a band with no bounds' },
  exDisaggregation: { id: 'app.explorer.disaggregation', defaultMessage: 'Disaggregation', description: 'Data explorer: what the figures are split by' },
  exNoDisaggregation: { id: 'app.explorer.noDisaggregation', defaultMessage: 'None (totals only)', description: 'Data explorer: the figures are not split' },
  exSeriesBy: { id: 'app.explorer.seriesBy', defaultMessage: 'Series', description: 'Data explorer: what each series of a chart is' },
  exSeriesBy_indicator: { id: 'app.explorer.seriesBy.indicator', defaultMessage: 'Indicators', description: 'Data explorer: a series per indicator' },
  exSeriesBy_place: { id: 'app.explorer.seriesBy.place', defaultMessage: 'Places', description: 'Data explorer: a series per place' },
  exSeriesBy_band: { id: 'app.explorer.seriesBy.band', defaultMessage: 'Bands', description: 'Data explorer: a series per disaggregation band' },
  exNotRecorded: { id: 'app.explorer.notRecorded', defaultMessage: 'Not recorded', description: 'Data explorer: the band of everything with no value for the disaggregation' },
  exChildAreas: { id: 'app.explorer.childAreas', defaultMessage: 'Each area under {place}', description: 'Data explorer: pick every place directly under the user’s place' },
  exBandHeader: { id: 'app.explorer.bandHeader', defaultMessage: 'Band', description: 'Data explorer: the table row header when the rows are disaggregation bands' },
  exFollowsPlace: { id: 'app.explorer.followsPlace', defaultMessage: 'Follows whoever views it: their own place.', description: 'Data explorer: the places follow the viewer' },
  exFollowsAreas: { id: 'app.explorer.followsAreas', defaultMessage: 'Follows whoever views it: each area under their place.', description: 'Data explorer: the places follow the viewer' },
  exSave: { id: 'app.explorer.save', defaultMessage: 'Save Visualization', description: 'Data explorer: save the Visualization shown' },
  exSaveTitle: { id: 'app.explorer.saveTitle', defaultMessage: 'Title', description: 'Data explorer: the saved Visualization’s title' },
  exSaveOnto: { id: 'app.explorer.saveOnto', defaultMessage: 'Dashboard', description: 'Data explorer: the Dashboard to save onto' },
  exSaveOntoNone: { id: 'app.explorer.saveOntoNone', defaultMessage: 'None', description: 'Data explorer: save onto no Dashboard' },
  exSaveOntoNew: { id: 'app.explorer.saveOntoNew', defaultMessage: 'New Dashboard…', description: 'Data explorer: save onto a new Dashboard' },
  exNewDashboardTitle: { id: 'app.explorer.newDashboardTitle', defaultMessage: 'New Dashboard’s title', description: 'Data explorer: the new Dashboard’s title' },
  exSaveConfirm: { id: 'app.explorer.saveConfirm', defaultMessage: 'Save', description: 'Data explorer: confirm saving' },
  exSaveCancel: { id: 'app.explorer.saveCancel', defaultMessage: 'Cancel', description: 'Data explorer: cancel saving' },
  exSaveFailed: { id: 'app.explorer.saveFailed', defaultMessage: 'The Visualization could not be saved. Please try again.', description: 'Data explorer: saving failed' },
  exSaved: { id: 'app.explorer.saved', defaultMessage: 'Visualization saved.', description: 'Data explorer: saved onto no Dashboard' },
  exSavedOnto: { id: 'app.explorer.savedOnto', defaultMessage: 'Visualization saved on {dashboard}.', description: 'Data explorer: saved onto a Dashboard' },
  dbTitle: { id: 'app.dashboards.title', defaultMessage: 'Dashboards', description: 'Dashboards heading and links' },
  dbLoading: { id: 'app.dashboards.loading', defaultMessage: 'Loading…', description: 'Dashboards: loading' },
  dbFailed: { id: 'app.dashboards.failed', defaultMessage: 'The Dashboards could not be loaded. Please try again.', description: 'Dashboards: loading failed' },
  dbForbidden: { id: 'app.dashboards.forbidden', defaultMessage: 'Dashboards need the reporting.view permission.', description: 'Dashboards: the user may not view them' },
  dbList: { id: 'app.dashboards.list', defaultMessage: 'Dashboards', description: 'Dashboards: the list on the left' },
  dbStandard: { id: 'app.dashboards.standard', defaultMessage: 'Standard', description: 'Dashboards: standard Dashboards' },
  dbNoStandard: { id: 'app.dashboards.noStandard', defaultMessage: 'None yet: standard Dashboards come with the Reporting package.', description: 'Dashboards: no standard Dashboards' },
  dbSaved: { id: 'app.dashboards.saved', defaultMessage: 'Saved', description: 'Dashboards: saved Dashboards' },
  dbNoSaved: { id: 'app.dashboards.noSaved', defaultMessage: 'None shared with you yet.', description: 'Dashboards: no saved Dashboards' },
  dbSavedAtCentral: { id: 'app.dashboards.savedAtCentral', defaultMessage: 'Saved Dashboards are kept at central. The standard ones work here offline, over this facility’s own figures.', description: 'Dashboards: on a Facility Edge Node, saved Dashboards are not here' },
  dbMine: { id: 'app.dashboards.mine', defaultMessage: 'Yours', description: 'Dashboards: a Dashboard the user made' },
  dbSharedWithYou: { id: 'app.dashboards.sharedWithYou', defaultMessage: 'Shared with you', description: 'Dashboards: a Dashboard shared with the user' },
  dbUnplaced: { id: 'app.dashboards.unplaced', defaultMessage: 'Your Visualizations on no Dashboard', description: 'Dashboards: saved Visualizations on no Dashboard' },
  dbPick: { id: 'app.dashboards.pick', defaultMessage: 'Pick a Dashboard on the left.', description: 'Dashboards: nothing picked' },
  dbEmpty: { id: 'app.dashboards.empty', defaultMessage: 'No Visualizations yet: save one from the Data explorer.', description: 'Dashboards: an empty Dashboard' },
  dbOpenInExplorer: { id: 'app.dashboards.openInExplorer', defaultMessage: 'Open in explorer', description: 'Dashboards: open an item in the data explorer' },
  dbNew: { id: 'app.dashboards.new', defaultMessage: 'New Dashboard', description: 'Dashboards: make a Dashboard' },
  dbNewTitle: { id: 'app.dashboards.newTitle', defaultMessage: 'Title', description: 'Dashboards: a new Dashboard’s title' },
  dbCreate: { id: 'app.dashboards.create', defaultMessage: 'Create', description: 'Dashboards: create the Dashboard' },
  dbSharing: { id: 'app.dashboards.sharing', defaultMessage: 'Sharing', description: 'Dashboards: who a Dashboard is shared with' },
  dbSharingIntro: { id: 'app.dashboards.sharingIntro', defaultMessage: 'Each viewer still sees only their own area.', description: 'Dashboards: sharing never widens what a viewer reads' },
  dbLevels: { id: 'app.dashboards.levels', defaultMessage: 'Every place of a level', description: 'Dashboards: share with a place level' },
  dbLevel_facility: { id: 'app.dashboards.level.facility', defaultMessage: 'Every facility', description: 'Dashboards: share with every facility' },
  dbLevel_district: { id: 'app.dashboards.level.district', defaultMessage: 'Every district', description: 'Dashboards: share with every district' },
  dbLevel_region: { id: 'app.dashboards.level.region', defaultMessage: 'Every region', description: 'Dashboards: share with every region' },
  dbLevel_national: { id: 'app.dashboards.level.national', defaultMessage: 'National', description: 'Dashboards: share with the national place' },
  dbRoles: { id: 'app.dashboards.roles', defaultMessage: 'Roles', description: 'Dashboards: share with roles' },
  dbPeople: { id: 'app.dashboards.people', defaultMessage: 'People (usernames)', description: 'Dashboards: share with people' },
  dbListHint: { id: 'app.dashboards.listHint', defaultMessage: 'Separate several with commas.', description: 'Dashboards: roles and people are comma-separated' },
  dbSaveSharing: { id: 'app.dashboards.saveSharing', defaultMessage: 'Save sharing', description: 'Dashboards: save who it is shared with' },
  dbSharingSaved: { id: 'app.dashboards.sharingSaved', defaultMessage: 'Sharing saved.', description: 'Dashboards: sharing saved' },
  dbChangeFailed: { id: 'app.dashboards.changeFailed', defaultMessage: 'The Dashboard could not be saved. Please try again.', description: 'Dashboards: a change failed' },
  dbSharedWith: { id: 'app.dashboards.sharedWith', defaultMessage: 'Shared with {shares}.', description: 'Dashboards: who the Dashboard is shared with' },
  dbNotShared: { id: 'app.dashboards.notShared', defaultMessage: 'Not shared: only you see it.', description: 'Dashboards: shared with nobody' },
  dbStandardFrom: { id: 'app.dashboards.standardFrom', defaultMessage: 'From {package}', description: 'Dashboards: the Reporting package version a standard Dashboard comes from' },
  dbStandardNote: { id: 'app.dashboards.standardNote', defaultMessage: 'A standard Dashboard: everyone sees it over their own area, and it changes only in a new version of its Reporting package.', description: 'Dashboards: a standard Dashboard cannot be edited in place' },
  dbSaveAs: { id: 'app.dashboards.saveAs', defaultMessage: 'Save as', description: 'Dashboards: save a standard Dashboard as your own copy' },
  dbSaveAsTitle: { id: 'app.dashboards.saveAsTitle', defaultMessage: 'Your copy’s title', description: 'Dashboards: the title of the copy Save as makes' },
  dbSaveAsCopy: { id: 'app.dashboards.saveAsCopy', defaultMessage: '{title} (copy)', description: 'Dashboards: the title Save as suggests' },
  dbSaveAsButton: { id: 'app.dashboards.saveAsButton', defaultMessage: 'Save as my Dashboard', description: 'Dashboards: make the copy' },
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
  sdElementChange: { summary: string; bump: string }
  sdNoteHeading: {}
  sdNoteLabel: {}
  sdNoteHint: {}
  sdNoteSave: {}
  sdNoteSaved: {}
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
  sdCommented: { by: string; at: string }
  sdCommentAbout: {}
  sdCommentLabel: {}
  sdCommentAboutLabel: {}
  sdCommentAboutNone: {}
  sdCommentPost: {}
  sdCommentPosted: {}
  sdCommentFailed: { reason: string }
  sdCommentClosed: { reason: string }
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
  fdImportFailed: { reason: string }
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
  sdElementChange: {
    id: 'app.studio.elementChange',
    defaultMessage: '{summary} ({bump, select, patch {needs a patch} minor {needs a minor version} major {needs a major version} other {needs no new version}})',
    description: 'Studio: one changed element of a changed artefact, with the version bump it needs',
  },
  sdNoteHeading: { id: 'app.studio.noteHeading', defaultMessage: 'Change note', description: 'Studio: the Draft Holder\'s note for the whole draft' },
  sdNoteLabel: { id: 'app.studio.noteLabel', defaultMessage: 'Change note for the whole draft', description: 'Studio: change note field' },
  sdNoteHint: { id: 'app.studio.noteHint', defaultMessage: 'Shown above the generated summaries. Leave it empty to remove it.', description: 'Studio: change note hint' },
  sdNoteSave: { id: 'app.studio.noteSave', defaultMessage: 'Save the note', description: 'Studio: action' },
  sdNoteSaved: { id: 'app.studio.noteSaved', defaultMessage: 'The change note is saved.', description: 'Studio: change note saved' },
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
  sdCommented: { id: 'app.studio.commented', defaultMessage: '{by} commented, {at}', description: 'Studio: a comment in the discussion' },
  sdCommentAbout: { id: 'app.studio.commentAbout', defaultMessage: 'About', description: 'Studio: before the artefact a comment names' },
  sdCommentLabel: { id: 'app.studio.commentLabel', defaultMessage: 'Comment', description: 'Studio: the new comment field' },
  sdCommentAboutLabel: { id: 'app.studio.commentAboutLabel', defaultMessage: 'About which artefact', description: 'Studio: which artefact a new comment is about' },
  sdCommentAboutNone: { id: 'app.studio.commentAboutNone', defaultMessage: 'The whole draft', description: 'Studio: a comment about no one artefact' },
  sdCommentPost: { id: 'app.studio.commentPost', defaultMessage: 'Add the comment', description: 'Studio: action' },
  sdCommentPosted: { id: 'app.studio.commentPosted', defaultMessage: 'Your comment is in the discussion.', description: 'Studio: a comment was added' },
  sdCommentFailed: { id: 'app.studio.commentFailed', defaultMessage: 'The comment was not added. {reason}', description: 'Studio: a comment was refused' },
  sdCommentClosed: { id: 'app.studio.commentClosed', defaultMessage: 'The discussion is closed. {reason}', description: 'Studio: no more comments on a published or withdrawn draft' },
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
  fdImportFailed: { id: 'app.studio.formDesigner.importFailed', defaultMessage: 'Central could not read it. {reason}', description: 'Studio: the import request failed' },
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
  ceAddCode: {}
  ceAddUnder: {}
  ceAddHeading: {}
  ceAddUnderHeading: { code: string }
  ceEditHeading: { code: string }
  ceCode: {}
  ceCodeFixed: {}
  ceDisplay: {}
  ceDefinition: {}
  ceParent: {}
  ceParentHint: {}
  ceNote: {}
  ceNoteHint: {}
  ceAdd: {}
  ceSave: {}
  ceCancel: {}
  ceDelete: { code: string }
  ceRetire: { code: string }
  ceRetired: {}
  ceRetiredHint: {}
  ceFailed: { reason: string }
  ceSaved: {}
  ceVersion: {}
  ceVersionFirst: { version: string }
  ceVersionSame: { published: string }
  ceVersionNext: { next: string; published: string; bump: string }
  ceReaches: {}
  ceReachesNone: {}
  ceReachValueSet: { title: string; package: string }
  ceGains: { codes: string }
  ceLoses: { codes: string }
  ceLosesHint: {}
  ceReachSame: {}
  ceReachBinding: { form: string; linkId: string }
  ceReachUnbound: {}
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
  // Editing a local CodeSystem's concepts (#524).
  ceAddCode: { id: 'app.studio.terminology.addCode', defaultMessage: 'Add a code', description: 'Studio: add a top-level code to a local CodeSystem' },
  ceAddUnder: { id: 'app.studio.terminology.addUnder', defaultMessage: '+ under', description: 'Studio: add a code under this concept (its code follows, for screen readers)' },
  ceAddHeading: { id: 'app.studio.terminology.addHeading', defaultMessage: 'New code', description: 'Studio: heading of the form adding a top-level code' },
  ceAddUnderHeading: { id: 'app.studio.terminology.addUnderHeading', defaultMessage: 'New code under {code}', description: 'Studio: heading of the form adding a code under another' },
  ceEditHeading: { id: 'app.studio.terminology.editHeading', defaultMessage: 'Code {code}', description: 'Studio: heading of the concept form' },
  ceCode: { id: 'app.studio.terminology.code', defaultMessage: 'Code', description: 'Studio: concept form field' },
  ceCodeFixed: { id: 'app.studio.terminology.codeFixed', defaultMessage: 'Published: its code and parent are fixed. It can be retired, not deleted.', description: 'Studio: why a published code cannot be renamed, moved or deleted' },
  ceDisplay: { id: 'app.studio.terminology.display', defaultMessage: 'Display', description: 'Studio: concept form field' },
  ceDefinition: { id: 'app.studio.terminology.definition', defaultMessage: 'Definition', description: 'Studio: concept form field' },
  ceParent: { id: 'app.studio.terminology.parent', defaultMessage: 'Parent code', description: 'Studio: concept form field' },
  ceParentHint: { id: 'app.studio.terminology.parentHint', defaultMessage: 'Empty for a top-level code.', description: 'Studio: concept form hint' },
  ceNote: { id: 'app.studio.terminology.note', defaultMessage: 'Note for reviewers', description: 'Studio: optional note on a concept change' },
  ceNoteHint: { id: 'app.studio.terminology.noteHint', defaultMessage: 'Optional: why, for example “IMCI 2024 adds this sign”. It goes to the draft\'s discussion.', description: 'Studio: concept change note hint' },
  ceAdd: { id: 'app.studio.terminology.add', defaultMessage: 'Add the code', description: 'Studio: submit a new code' },
  ceSave: { id: 'app.studio.terminology.saveCode', defaultMessage: 'Save the code', description: 'Studio: save a changed code' },
  ceCancel: { id: 'app.studio.terminology.cancel', defaultMessage: 'Cancel', description: 'Studio: close the new code form' },
  ceDelete: { id: 'app.studio.terminology.delete', defaultMessage: 'Delete {code}', description: 'Studio: delete a new code' },
  ceRetire: { id: 'app.studio.terminology.retire', defaultMessage: 'Retire {code}', description: 'Studio: retire a published code' },
  ceRetired: { id: 'app.studio.terminology.retired', defaultMessage: 'Retired', description: 'Studio: a retired code' },
  ceRetiredHint: { id: 'app.studio.terminology.retiredHint', defaultMessage: 'Retired: records that hold it still resolve, and new forms no longer offer it.', description: 'Studio: what retiring a code means' },
  ceFailed: { id: 'app.studio.terminology.codeFailed', defaultMessage: 'The code was not saved: {reason}', description: 'Studio: a concept edit failed' },
  ceSaved: { id: 'app.studio.terminology.codeSaved', defaultMessage: 'Saved.', description: 'Studio: a concept edit was saved' },
  ceVersion: { id: 'app.studio.terminology.version', defaultMessage: 'Version', description: 'Studio: heading of the code system\'s next version' },
  ceVersionFirst: { id: 'app.studio.terminology.versionFirst', defaultMessage: 'Never published: it keeps its version {version}.', description: 'Studio: a code system never published' },
  ceVersionSame: { id: 'app.studio.terminology.versionSame', defaultMessage: 'No change from the published {published}.', description: 'Studio: no change against published' },
  ceVersionNext: {
    id: 'app.studio.terminology.versionNext',
    defaultMessage: 'Next version {next}, {bump, select, patch {a patch} minor {a minor version} major {a major version} other {no new version}} from the published {published}, because:',
    description: 'Studio: the next version of a code system and its bump',
  },
  ceReaches: { id: 'app.studio.terminology.reaches', defaultMessage: 'What it reaches', description: 'Studio: heading of the ValueSets a code system change reaches' },
  ceReachesNone: { id: 'app.studio.terminology.reachesNone', defaultMessage: 'No ValueSet draws on this code system.', description: 'Studio: nothing reached' },
  ceReachValueSet: { id: 'app.studio.terminology.reachValueSet', defaultMessage: '{title} ({package})', description: 'Studio: a ValueSet reached' },
  ceGains: { id: 'app.studio.terminology.gains', defaultMessage: 'Gains {codes}', description: 'Studio: codes a ValueSet gains' },
  ceLoses: { id: 'app.studio.terminology.loses', defaultMessage: 'Loses {codes}', description: 'Studio: codes a ValueSet loses' },
  ceLosesHint: { id: 'app.studio.terminology.losesHint', defaultMessage: 'Records that hold them still resolve; new forms no longer offer them.', description: 'Studio: what losing a code means' },
  ceReachSame: { id: 'app.studio.terminology.reachSame', defaultMessage: 'No change for new forms.', description: 'Studio: a ValueSet that neither gains nor loses' },
  ceReachBinding: { id: 'app.studio.terminology.reachBinding', defaultMessage: 'Bound by {form}, question {linkId}', description: 'Studio: a form binding a reached ValueSet' },
  ceReachUnbound: { id: 'app.studio.terminology.reachUnbound', defaultMessage: 'No form binds it.', description: 'Studio: a reached ValueSet no form binds' },
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
  wsPGuardTable: { table: string }
  wsPGuardOutcome: { table: string; outcome: string }
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
  wsGuardNoTables: {}
  wsGuardTable: {}
  wsGuardOutcome: {}
  wsGuardTableHint: {}
  wsGuardOpenTable: { table: string }
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
  wsTypedAction: { n: number }
  wsNotInRelease: { action: string }
  wsNeeds: { permission: string }
  wsCompensation: { kind: string; action: string }
  wsParam: { name: string; n: number; required: string }
  wsParamUnset: {}
  wsAddTyped: {}
  wsTypedToAdd: {}
  wsNoActions: {}
  wsActionsLoading: {}
  wsActionsFailed: {}
  wsPBadReference: { action: string }
  wsPUnknownAction: { action: string }
  wsPUnknownParam: { action: string; param: string }
  wsPMissingParam: { action: string; param: string }
  wsPInvalidParam: { action: string; param: string }
  wsPPrincipalLacks: { action: string; permission: string }
  wsPUnneeded: { permission: string }
  wsDeclare: { permission: string }
  wsUndeclare: { permission: string }
  wsPrincipal: { count: number; permissions: string }
  wsPrincipalHint: {}
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
  wsProblemsHint: { id: 'app.studio.workflow.problemsHint', defaultMessage: 'Rerun on every edit; each problem selects what it is about. Check now on the package page remains the authority: it also runs the scenarios.', description: 'Workflow Studio: publication checks hint' },
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
  wsPGuardTable: { id: 'app.studio.workflow.pGuardTable', defaultMessage: 'A guard names the decision table “{table}”, which the definition does not have', description: 'Workflow Studio: problem' },
  wsPGuardOutcome: { id: 'app.studio.workflow.pGuardOutcome', defaultMessage: 'A guard requires the outcome “{outcome}” of the decision table “{table}”, which it never gives', description: 'Workflow Studio: problem' },
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
  wsGuardName: { id: 'app.studio.workflow.guardName', defaultMessage: '{guard, select, always {Always} fhirpath {A FHIRPath expression} table {A decision table’s outcome} other {{guard}}}', description: 'Workflow Studio: a guard kind' },
  wsExpression: { id: 'app.studio.workflow.expression', defaultMessage: 'FHIRPath expression', description: 'Workflow Studio: field' },
  wsGuardNoTables: { id: 'app.studio.workflow.guardNoTables', defaultMessage: 'Add a decision table to guard on its outcome.', description: 'Workflow Studio: no decision table to guard on' },
  wsGuardTable: { id: 'app.studio.workflow.guardTable', defaultMessage: 'Decision table', description: 'Workflow Studio: field' },
  wsGuardOutcome: { id: 'app.studio.workflow.guardOutcome', defaultMessage: 'Required outcome', description: 'Workflow Studio: field' },
  wsGuardTableHint: {
    id: 'app.studio.workflow.guardTableHint',
    defaultMessage: 'Taken only when the table gives this outcome: its first rule that holds decides, else its default; with neither, the guard does not hold.',
    description: 'Workflow Studio: what a decision-table guard means',
  },
  wsGuardOpenTable: { id: 'app.studio.workflow.guardOpenTable', defaultMessage: 'Open {table} in the decision-table editor', description: 'Workflow Studio: open the guard’s table' },
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
  wsTypedAction: { id: 'app.studio.workflow.typedAction', defaultMessage: 'Typed action {n}', description: 'Workflow Studio: the action a typed action executes' },
  wsNotInRelease: { id: 'app.studio.workflow.notInRelease', defaultMessage: '{action} (not in this release)', description: 'Workflow Studio: an action reference the catalogue lacks' },
  wsNeeds: { id: 'app.studio.workflow.needs', defaultMessage: 'Needs the permission {permission}', description: 'Workflow Studio: a typed action’s permission' },
  wsCompensation: {
    id: 'app.studio.workflow.compensation',
    defaultMessage: '{kind, select, none {Cannot be undone: a cancellation records why} action {Undone by {action}} humantask {Undone by a person, as a human task} other {}}',
    description: 'Workflow Studio: a typed action’s compensation',
  },
  wsParam: { id: 'app.studio.workflow.param', defaultMessage: '{name} of action {n}{required, select, yes { (required)} other {}}', description: 'Workflow Studio: a parameter field' },
  wsParamUnset: { id: 'app.studio.workflow.paramUnset', defaultMessage: 'Not set', description: 'Workflow Studio: an optional code parameter left out' },
  wsAddTyped: { id: 'app.studio.workflow.addTyped', defaultMessage: 'Add: typed action', description: 'Workflow Studio: button' },
  wsTypedToAdd: { id: 'app.studio.workflow.typedToAdd', defaultMessage: 'Typed action to add', description: 'Workflow Studio: field' },
  wsNoActions: { id: 'app.studio.workflow.noActions', defaultMessage: 'No typed actions are registered in this release.', description: 'Workflow Studio: empty catalogue' },
  wsActionsLoading: { id: 'app.studio.workflow.actionsLoading', defaultMessage: 'Reading this release’s typed actions…', description: 'Workflow Studio: loading the catalogue' },
  wsActionsFailed: { id: 'app.studio.workflow.actionsFailed', defaultMessage: 'This release’s typed actions could not be read, so they are not checked here.', description: 'Workflow Studio: the catalogue failed' },
  wsPBadReference: { id: 'app.studio.workflow.pBadReference', defaultMessage: '“{action}” is not an action reference (id@major)', description: 'Workflow Studio: problem' },
  wsPUnknownAction: { id: 'app.studio.workflow.pUnknownAction', defaultMessage: 'This release has no typed action “{action}”', description: 'Workflow Studio: problem' },
  wsPUnknownParam: { id: 'app.studio.workflow.pUnknownParam', defaultMessage: '“{action}” has no parameter “{param}”', description: 'Workflow Studio: problem' },
  wsPMissingParam: { id: 'app.studio.workflow.pMissingParam', defaultMessage: '“{action}” needs its parameter “{param}”', description: 'Workflow Studio: problem' },
  wsPInvalidParam: { id: 'app.studio.workflow.pInvalidParam', defaultMessage: 'The parameter “{param}” of “{action}” is not valid', description: 'Workflow Studio: problem' },
  wsPPrincipalLacks: {
    id: 'app.studio.workflow.pPrincipalLacks',
    defaultMessage: 'A timer or eventless step runs “{action}” as the workflow principal, which lacks “{permission}”',
    description: 'Workflow Studio: problem',
  },
  wsPUnneeded: { id: 'app.studio.workflow.pUnneeded', defaultMessage: 'The workflow principal holds “{permission}”, which no action needs', description: 'Workflow Studio: problem' },
  wsDeclare: { id: 'app.studio.workflow.declare', defaultMessage: 'Declare “{permission}” for the workflow principal', description: 'Workflow Studio: button on a problem' },
  wsUndeclare: { id: 'app.studio.workflow.undeclare', defaultMessage: 'Remove “{permission}” from the workflow principal', description: 'Workflow Studio: button on a problem' },
  wsPrincipal: { id: 'app.studio.workflow.principal', defaultMessage: '{count, plural, =0 {The workflow principal holds no permissions.} other {The workflow principal holds {permissions}.}}', description: 'Workflow Studio: the principal’s declared permissions' },
  wsPrincipalHint: {
    id: 'app.studio.workflow.principalHint',
    defaultMessage: 'Timer and eventless steps run as the workflow principal, with only these permissions. Those declared here are approved at publication.',
    description: 'Workflow Studio: hint',
  },
  wsAddRaise: { id: 'app.studio.workflow.addRaise', defaultMessage: 'Add: raise an event', description: 'Workflow Studio: button' },
  wsAddAssign: { id: 'app.studio.workflow.addAssign', defaultMessage: 'Add: set a field', description: 'Workflow Studio: button' },
  wsRemoveAction: { id: 'app.studio.workflow.removeAction', defaultMessage: 'Remove action {n}', description: 'Workflow Studio: button' },
})

type DecisionValues = {
  dtTables: {}
  dtTablesHint: {}
  dtTablesNone: {}
  dtTableRow: { id: string; count: number }
  dtAddTable: {}
  dtHeading: { id: string }
  dtBack: {}
  dtId: {}
  dtDefault: {}
  dtDefaultHint: {}
  dtChecks: {}
  dtChecksHint: {}
  dtChecking: {}
  dtCheckFailed: { reason: string }
  dtNoRules: {}
  dtGaps: { count: number; example: string }
  dtTooMany: { count: number; most: number }
  dtNeverFiresCheck: { count: number; names: string }
  dtTestsResult: { count: number; failing: number }
  dtError: { error: string }
  dtInputs: { count: number }
  dtInputsHint: {}
  dtRequirement: { name: string; resourceType: string }
  dtRequirementName: {}
  dtResourceType: {}
  dtAddRequirement: {}
  dtPath: { name: string }
  dtAddPath: { name: string }
  dtRemovePath: { path: string }
  dtRules: {}
  dtRulesHint: {}
  dtRuleRow: { n: number; name: string }
  dtThen: { outcome: string }
  dtFires: {}
  dtNever: {}
  dtUp: { name: string }
  dtDown: { name: string }
  dtAddRule: {}
  dtNewRule: { n: number }
  dtRuleNone: {}
  dtChoose: {}
  dtRule: { n: number }
  dtRuleName: {}
  dtOutcome: {}
  dtDeleteRule: {}
  dtCompiled: {}
  dtHolds: {}
  dtHoldsNot: {}
  dtRaw: { at: string }
  dtRawHint: {}
  dtInput: { at: string }
  dtOperator: { at: string }
  dtKind: { at: string }
  dtKindName: { kind: string }
  dtValue: { at: string }
  dtRemoveCondition: { at: string }
  dtGroup: { at: string; join: string }
  dtJoin: { at: string }
  dtJoinName: { join: string }
  dtAddCondition: { at: string }
  dtAddGroup: { at: string }
  dtRemoveGroup: { at: string }
  dtEmptyGroup: {}
  dtTry: {}
  dtTryHint: {}
  dtTryNothing: {}
  dtTrying: {}
  dtTryFailed: { reason: string }
  dtNoOutcome: {}
  dtOutcomeDefault: { outcome: string }
  dtOutcomeFrom: { outcome: string }
  dtTestName: {}
  dtSaveTest: {}
  dtTestSaved: { path: string }
  dtTestFailed: { reason: string }
  dtTestCases: {}
  dtTestsNone: {}
  dtTestCase: { name: string; expect: string; outcome: string }
  dtPasses: {}
  dtFails: {}
  dtNothing: {}
}

export const decisionMessages = defineMessages<DecisionValues>({
  dtTables: { id: 'app.studio.decision.tables', defaultMessage: 'Decision tables', description: 'Decision tables: heading in the Workflow Studio' },
  dtTablesHint: { id: 'app.studio.decision.tablesHint', defaultMessage: 'Named rules tried from the top: the first that holds gives the outcome.', description: 'Decision tables: hint' },
  dtTablesNone: { id: 'app.studio.decision.tablesNone', defaultMessage: 'No decision tables yet.', description: 'Decision tables: none' },
  dtTableRow: { id: 'app.studio.decision.tableRow', defaultMessage: '{id}: {count, plural, one {# rule} other {# rules}}', description: 'Decision tables: one table, to open it' },
  dtAddTable: { id: 'app.studio.decision.addTable', defaultMessage: 'New decision table', description: 'Decision tables: button' },
  dtHeading: { id: 'app.studio.decision.heading', defaultMessage: 'Decision table {id}', description: 'Decision tables: editor heading' },
  dtBack: { id: 'app.studio.decision.back', defaultMessage: 'Back to the statechart', description: 'Decision tables: close the editor' },
  dtId: { id: 'app.studio.decision.id', defaultMessage: 'Table id', description: 'Decision tables: field' },
  dtDefault: { id: 'app.studio.decision.default', defaultMessage: 'When no rule holds', description: 'Decision tables: the default outcome' },
  dtDefaultHint: { id: 'app.studio.decision.defaultHint', defaultMessage: 'Empty: no outcome, so what no rule covers is a gap.', description: 'Decision tables: hint' },
  dtChecks: { id: 'app.studio.decision.checks', defaultMessage: 'Checks', description: 'Decision tables: heading' },
  dtChecksHint: {
    id: 'app.studio.decision.checksHint',
    defaultMessage: 'The server reruns them as you edit, over every combination of the values the rules name. Check now on the package page remains the authority.',
    description: 'Decision tables: hint',
  },
  dtChecking: { id: 'app.studio.decision.checking', defaultMessage: 'Checking…', description: 'Decision tables: checks loading' },
  dtCheckFailed: { id: 'app.studio.decision.checkFailed', defaultMessage: 'The checks could not run: {reason}', description: 'Decision tables: checks failed' },
  dtNoRules: { id: 'app.studio.decision.noRules', defaultMessage: 'The table has no rules yet: add one.', description: 'Decision tables: no rules' },
  dtGaps: {
    id: 'app.studio.decision.gaps',
    defaultMessage: '{count, plural, =0 {Every combination is covered} one {# combination matches no rule, e.g. {example}} other {# combinations match no rule, e.g. {example}}}',
    description: 'Decision tables: coverage',
  },
  dtTooMany: { id: 'app.studio.decision.tooMany', defaultMessage: 'Too many combinations to check coverage: {count}, at most {most}.', description: 'Decision tables: coverage not checked' },
  dtNeverFiresCheck: {
    id: 'app.studio.decision.neverFiresCheck',
    defaultMessage: '{count, plural, =0 {Every rule can fire} other {Never fires, a rule above always matches first: {names}}}',
    description: 'Decision tables: rules that never fire',
  },
  dtTestsResult: {
    id: 'app.studio.decision.testsResult',
    defaultMessage: '{count, plural, =0 {No saved test cases} one {{failing} of # test case fails} other {{failing} of # test cases fail}}',
    description: 'Decision tables: test cases',
  },
  dtError: { id: 'app.studio.decision.error', defaultMessage: 'Cannot be evaluated: {error}', description: 'Decision tables: a condition that cannot be evaluated' },
  dtInputs: {
    id: 'app.studio.decision.inputs',
    defaultMessage: 'Inputs: {count, plural, =0 {none declared} one {# from the data requirements} other {# from the data requirements}}',
    description: 'Decision tables: inputs heading',
  },
  dtInputsHint: { id: 'app.studio.decision.inputsHint', defaultMessage: 'Each path a data requirement declares is one input, read as %<name>.<path>.', description: 'Decision tables: hint' },
  dtRequirement: { id: 'app.studio.decision.requirement', defaultMessage: '%{name} ({resourceType})', description: 'Decision tables: one data requirement' },
  dtRequirementName: { id: 'app.studio.decision.requirementName', defaultMessage: 'Name', description: 'Decision tables: field' },
  dtResourceType: { id: 'app.studio.decision.resourceType', defaultMessage: 'Resource type', description: 'Decision tables: field' },
  dtAddRequirement: { id: 'app.studio.decision.addRequirement', defaultMessage: 'Add a data requirement', description: 'Decision tables: button' },
  dtPath: { id: 'app.studio.decision.path', defaultMessage: 'Path to add to {name}', description: 'Decision tables: field' },
  dtAddPath: { id: 'app.studio.decision.addPath', defaultMessage: 'Add the path to {name}', description: 'Decision tables: button' },
  dtRemovePath: { id: 'app.studio.decision.removePath', defaultMessage: 'Remove {path}', description: 'Decision tables: button' },
  dtRules: { id: 'app.studio.decision.rules', defaultMessage: 'Rules', description: 'Decision tables: heading' },
  dtRulesHint: { id: 'app.studio.decision.rulesHint', defaultMessage: 'Tried from the top; the first that holds wins.', description: 'Decision tables: hint' },
  dtRuleRow: { id: 'app.studio.decision.ruleRow', defaultMessage: '{n}. {name}', description: 'Decision tables: a rule in the list' },
  dtThen: { id: 'app.studio.decision.then', defaultMessage: '→ {outcome}', description: 'Decision tables: a rule outcome' },
  dtFires: { id: 'app.studio.decision.fires', defaultMessage: 'fires for this patient', description: 'Decision tables: marker' },
  dtNever: { id: 'app.studio.decision.never', defaultMessage: 'never fires', description: 'Decision tables: marker' },
  dtUp: { id: 'app.studio.decision.up', defaultMessage: 'Move “{name}” up', description: 'Decision tables: button' },
  dtDown: { id: 'app.studio.decision.down', defaultMessage: 'Move “{name}” down', description: 'Decision tables: button' },
  dtAddRule: { id: 'app.studio.decision.addRule', defaultMessage: 'Add rule', description: 'Decision tables: button' },
  dtNewRule: { id: 'app.studio.decision.newRule', defaultMessage: 'New rule {n}', description: 'Decision tables: the name of a new rule' },
  dtRuleNone: { id: 'app.studio.decision.ruleNone', defaultMessage: 'No rule open', description: 'Decision tables: heading' },
  dtChoose: { id: 'app.studio.decision.choose', defaultMessage: 'Choose a rule from the list to see and edit it.', description: 'Decision tables: hint' },
  dtRule: { id: 'app.studio.decision.rule', defaultMessage: 'Rule {n}', description: 'Decision tables: heading' },
  dtRuleName: { id: 'app.studio.decision.ruleName', defaultMessage: 'Rule name', description: 'Decision tables: field' },
  dtOutcome: { id: 'app.studio.decision.outcome', defaultMessage: 'Outcome', description: 'Decision tables: field' },
  dtDeleteRule: { id: 'app.studio.decision.deleteRule', defaultMessage: 'Delete rule', description: 'Decision tables: button' },
  dtCompiled: { id: 'app.studio.decision.compiled', defaultMessage: 'Saved as FHIRPath', description: 'Decision tables: the compiled expression' },
  dtHolds: { id: 'app.studio.decision.holds', defaultMessage: 'holds for this patient', description: 'Decision tables: marker' },
  dtHoldsNot: { id: 'app.studio.decision.holdsNot', defaultMessage: 'does not hold for this patient', description: 'Decision tables: marker' },
  dtRaw: { id: 'app.studio.decision.raw', defaultMessage: 'FHIRPath {at}', description: 'Decision tables: a kept expression' },
  dtRawHint: { id: 'app.studio.decision.rawHint', defaultMessage: 'Kept as written.', description: 'Decision tables: hint' },
  dtInput: { id: 'app.studio.decision.input', defaultMessage: 'Input of condition {at}', description: 'Decision tables: field' },
  dtOperator: { id: 'app.studio.decision.operator', defaultMessage: 'Operator of condition {at}', description: 'Decision tables: field' },
  dtKind: { id: 'app.studio.decision.kind', defaultMessage: 'Type of condition {at}', description: 'Decision tables: field' },
  dtKindName: { id: 'app.studio.decision.kindName', defaultMessage: '{kind, select, boolean {yes or no} number {number} other {code}}', description: 'Decision tables: a value type' },
  dtValue: { id: 'app.studio.decision.value', defaultMessage: 'Value of condition {at}', description: 'Decision tables: field' },
  dtRemoveCondition: { id: 'app.studio.decision.removeCondition', defaultMessage: 'Remove condition {at}', description: 'Decision tables: button' },
  dtGroup: { id: 'app.studio.decision.group', defaultMessage: 'Group {at}: {join, select, all {all of these} any {any of these} other {none of these}}', description: 'Decision tables: a group' },
  dtJoin: { id: 'app.studio.decision.join', defaultMessage: 'Join of group {at}', description: 'Decision tables: field' },
  dtJoinName: { id: 'app.studio.decision.joinName', defaultMessage: '{join, select, all {all of these} any {any of these} other {not (none of these)}}', description: 'Decision tables: a join' },
  dtAddCondition: { id: 'app.studio.decision.addCondition', defaultMessage: 'Add a condition to group {at}', description: 'Decision tables: button' },
  dtAddGroup: { id: 'app.studio.decision.addGroup', defaultMessage: 'Add a group to group {at}', description: 'Decision tables: button' },
  dtRemoveGroup: { id: 'app.studio.decision.removeGroup', defaultMessage: 'Remove group {at}', description: 'Decision tables: button' },
  dtEmptyGroup: { id: 'app.studio.decision.emptyGroup', defaultMessage: 'Empty: it always holds. Add a condition or a group.', description: 'Decision tables: hint' },
  dtTry: { id: 'app.studio.decision.try', defaultMessage: 'Try a patient', description: 'Decision tables: heading' },
  dtTryHint: { id: 'app.studio.decision.tryHint', defaultMessage: 'The server decides the draft table for these inputs, and marks each part of the open rule ✓ or ✗.', description: 'Decision tables: hint' },
  dtTryNothing: { id: 'app.studio.decision.tryNothing', defaultMessage: 'No rule reads a declared input yet.', description: 'Decision tables: nothing to try' },
  dtTrying: { id: 'app.studio.decision.trying', defaultMessage: 'Deciding…', description: 'Decision tables: try loading' },
  dtTryFailed: { id: 'app.studio.decision.tryFailed', defaultMessage: 'It could not be decided: {reason}', description: 'Decision tables: try failed' },
  dtNoOutcome: { id: 'app.studio.decision.noOutcome', defaultMessage: 'No rule holds and there is no default', description: 'Decision tables: no outcome' },
  dtOutcomeDefault: { id: 'app.studio.decision.outcomeDefault', defaultMessage: 'Outcome: {outcome}, the default', description: 'Decision tables: the default decided' },
  dtOutcomeFrom: { id: 'app.studio.decision.outcomeFrom', defaultMessage: 'Outcome: {outcome}, from', description: 'Decision tables: a rule decided; the rule follows' },
  dtTestName: { id: 'app.studio.decision.testName', defaultMessage: 'Name this case', description: 'Decision tables: field' },
  dtSaveTest: { id: 'app.studio.decision.saveTest', defaultMessage: 'Save as test', description: 'Decision tables: button' },
  dtTestSaved: { id: 'app.studio.decision.testSaved', defaultMessage: 'Saved as {path}. Submission runs it.', description: 'Decision tables: test saved' },
  dtTestFailed: { id: 'app.studio.decision.testFailed', defaultMessage: 'The test case was not saved: {reason}', description: 'Decision tables: test not saved' },
  dtTestCases: { id: 'app.studio.decision.testCases', defaultMessage: 'Test cases', description: 'Decision tables: heading' },
  dtTestsNone: { id: 'app.studio.decision.testsNone', defaultMessage: 'No saved test cases for this table.', description: 'Decision tables: none' },
  dtTestCase: { id: 'app.studio.decision.testCase', defaultMessage: '{name}: expects {expect}, gets {outcome}', description: 'Decision tables: one test case' },
  dtPasses: { id: 'app.studio.decision.passes', defaultMessage: 'Passes:', description: 'Decision tables: test passes' },
  dtFails: { id: 'app.studio.decision.fails', defaultMessage: 'Fails:', description: 'Decision tables: test fails' },
  dtNothing: { id: 'app.studio.decision.nothing', defaultMessage: 'nothing', description: 'Decision tables: no outcome' },
})

type SandboxValues = {
  sbMode: {}
  sbModeName: { mode: string }
  sbHeading: {}
  sbHint: {}
  sbPlaying: {}
  sbFailed: { reason: string }
  sbUnanswered: { guards: string }
  sbRefused: { reason: string }
  sbGuards: {}
  sbGuardsNone: {}
  sbAnswer: { guard: string }
  sbAnswerName: { answer: string }
  sbGates: {}
  sbUnansweredTables: { tables: string }
  sbTables: {}
  sbTablesHint: {}
  sbTable: { table: string }
  sbGate: { gate: string }
  sbOutcomeName: { outcome: string }
  sbNow: { step: number }
  sbConfiguration: {}
  sbActions: {}
  sbTasks: {}
  sbTask: { state: string; status: string }
  sbTimers: {}
  sbTimer: { event: string; dueIn: string }
  sbNone: {}
  sbElapsed: { elapsed: string }
  sbCompleted: {}
  sbEvents: {}
  sbEventsNone: {}
  sbSend: { event: string }
  sbWaitFor: { event: string; dueIn: string }
  sbWait: {}
  sbWaitButton: {}
  sbUndo: {}
  sbRestart: {}
  sbHistory: {}
  sbHistoryStart: { configuration: string }
  sbHistoryMove: { kind: string; event: string; wait: string; configuration: string }
  sbHistoryNotTaken: { kind: string; event: string; wait: string }
  sbScenarioName: {}
  sbSave: {}
  sbSaved: { path: string }
  sbSaveFailed: { reason: string }
  sbActive: {}
}

export const sandboxMessages = defineMessages<SandboxValues>({
  sbMode: { id: 'app.studio.sandbox.mode', defaultMessage: 'Right panel', description: 'Workflow sandbox: the group of mode buttons' },
  sbModeName: { id: 'app.studio.sandbox.modeName', defaultMessage: '{mode, select, sandbox {Sandbox} other {Inspect}}', description: 'Workflow sandbox: a mode button' },
  sbHeading: { id: 'app.studio.sandbox.heading', defaultMessage: 'Sandbox', description: 'Workflow sandbox: heading' },
  sbHint: { id: 'app.studio.sandbox.hint', defaultMessage: 'Central plays the draft, unsaved edits included, with every action stubbed: nothing is carried out. Answer each guard; a move that needs an unanswered guard is not taken.', description: 'Workflow sandbox: hint' },
  sbPlaying: { id: 'app.studio.sandbox.playing', defaultMessage: 'Playing…', description: 'Workflow sandbox: waiting for the server' },
  sbFailed: { id: 'app.studio.sandbox.failed', defaultMessage: 'The sandbox could not play the draft: {reason}', description: 'Workflow sandbox: request failed' },
  sbUnanswered: { id: 'app.studio.sandbox.unanswered', defaultMessage: 'The next move was not taken: answer {guards} first.', description: 'Workflow sandbox: unanswered guards' },
  sbRefused: { id: 'app.studio.sandbox.refused', defaultMessage: 'The next move was not taken: {reason}', description: 'Workflow sandbox: move refused' },
  sbGuards: { id: 'app.studio.sandbox.guards', defaultMessage: 'Guard answers', description: 'Workflow sandbox: heading' },
  sbGuardsNone: { id: 'app.studio.sandbox.guardsNone', defaultMessage: 'The definition has no guards.', description: 'Workflow sandbox: no guards' },
  sbAnswer: { id: 'app.studio.sandbox.answer', defaultMessage: 'Answer to {guard}', description: 'Workflow sandbox: field' },
  sbAnswerName: { id: 'app.studio.sandbox.answerName', defaultMessage: '{answer, select, true {Holds} false {Does not hold} other {Not answered}}', description: 'Workflow sandbox: a guard answer' },
  sbGates: { id: 'app.studio.sandbox.gates', defaultMessage: 'Payment gates', description: 'Workflow sandbox: heading' },
  sbUnansweredTables: { id: 'app.studio.sandbox.unansweredTables', defaultMessage: 'The next move was not taken: give the outcome of {tables} first.', description: 'Workflow sandbox: unanswered decision tables' },
  sbTables: { id: 'app.studio.sandbox.tables', defaultMessage: 'Decision table outcomes', description: 'Workflow sandbox: heading' },
  sbTablesHint: { id: 'app.studio.sandbox.tablesHint', defaultMessage: 'A guard naming a table holds when the table gives the outcome it requires.', description: 'Workflow sandbox: what a table outcome answers' },
  sbTable: { id: 'app.studio.sandbox.table', defaultMessage: 'Outcome of {table}', description: 'Workflow sandbox: field' },
  sbGate: { id: 'app.studio.sandbox.gate', defaultMessage: 'Outcome of {gate}', description: 'Workflow sandbox: field' },
  sbOutcomeName: { id: 'app.studio.sandbox.outcomeName', defaultMessage: '{outcome, select, pass {Pass} block {Block} other {Defer}}', description: 'Workflow sandbox: a gate outcome' },
  sbNow: { id: 'app.studio.sandbox.now', defaultMessage: '{step, plural, =0 {At the start} other {After move #}}', description: 'Workflow sandbox: heading of the current step' },
  sbConfiguration: { id: 'app.studio.sandbox.configuration', defaultMessage: 'Active states', description: 'Workflow sandbox: heading' },
  sbActions: { id: 'app.studio.sandbox.actions', defaultMessage: 'Actions (stubbed)', description: 'Workflow sandbox: heading' },
  sbTasks: { id: 'app.studio.sandbox.tasks', defaultMessage: 'Open tasks', description: 'Workflow sandbox: heading' },
  sbTask: { id: 'app.studio.sandbox.task', defaultMessage: '{state} ({status})', description: 'Workflow sandbox: an open task' },
  sbTimers: { id: 'app.studio.sandbox.timers', defaultMessage: 'Pending timers', description: 'Workflow sandbox: heading' },
  sbTimer: { id: 'app.studio.sandbox.timer', defaultMessage: '{event} in {dueIn}', description: 'Workflow sandbox: a pending timer' },
  sbNone: { id: 'app.studio.sandbox.none', defaultMessage: 'None', description: 'Workflow sandbox: an empty list' },
  sbElapsed: { id: 'app.studio.sandbox.elapsed', defaultMessage: 'Time since the start: {elapsed}', description: 'Workflow sandbox: elapsed time' },
  sbCompleted: { id: 'app.studio.sandbox.completed', defaultMessage: 'The instance is complete.', description: 'Workflow sandbox: the instance finished' },
  sbEvents: { id: 'app.studio.sandbox.events', defaultMessage: 'Send an event', description: 'Workflow sandbox: heading' },
  sbEventsNone: { id: 'app.studio.sandbox.eventsNone', defaultMessage: 'No active state takes an event.', description: 'Workflow sandbox: no events' },
  sbSend: { id: 'app.studio.sandbox.send', defaultMessage: 'Send {event}', description: 'Workflow sandbox: button' },
  sbWaitFor: { id: 'app.studio.sandbox.waitFor', defaultMessage: 'Wait {dueIn} for {event}', description: 'Workflow sandbox: button' },
  sbWait: { id: 'app.studio.sandbox.wait', defaultMessage: 'Wait (ISO 8601, e.g. PT30M)', description: 'Workflow sandbox: field' },
  sbWaitButton: { id: 'app.studio.sandbox.waitButton', defaultMessage: 'Wait', description: 'Workflow sandbox: button' },
  sbUndo: { id: 'app.studio.sandbox.undo', defaultMessage: 'Undo', description: 'Workflow sandbox: button' },
  sbRestart: { id: 'app.studio.sandbox.restart', defaultMessage: 'Restart', description: 'Workflow sandbox: button' },
  sbHistory: { id: 'app.studio.sandbox.history', defaultMessage: 'History', description: 'Workflow sandbox: heading' },
  sbHistoryStart: { id: 'app.studio.sandbox.historyStart', defaultMessage: 'Started in {configuration}', description: 'Workflow sandbox: history entry' },
  sbHistoryMove: { id: 'app.studio.sandbox.historyMove', defaultMessage: '{kind, select, wait {Waited {wait}} other {Sent {event}}} → {configuration}', description: 'Workflow sandbox: history entry' },
  sbHistoryNotTaken: { id: 'app.studio.sandbox.historyNotTaken', defaultMessage: '{kind, select, wait {Waited {wait}} other {Sent {event}}}: not taken', description: 'Workflow sandbox: history entry not taken' },
  sbScenarioName: { id: 'app.studio.sandbox.scenarioName', defaultMessage: 'Scenario name', description: 'Workflow sandbox: field' },
  sbSave: { id: 'app.studio.sandbox.save', defaultMessage: 'Save as scenario', description: 'Workflow sandbox: button' },
  sbSaved: { id: 'app.studio.sandbox.saved', defaultMessage: 'Saved as {path}. Submission runs it.', description: 'Workflow sandbox: scenario saved' },
  sbSaveFailed: { id: 'app.studio.sandbox.saveFailed', defaultMessage: 'The scenario was not saved: {reason}', description: 'Workflow sandbox: scenario not saved' },
  sbActive: { id: 'app.studio.sandbox.active', defaultMessage: 'active', description: 'Workflow sandbox: an active state in the outline' },
})

type ProgramValues = {
  pdOpen: { path: string }
  pdOpenShort: {}
  pdHeading: { title: string }
  pdLoading: {}
  pdRefused: { refusal: string }
  pdFile: { path: string; url: string; version: string }
  pdStrip: {}
  pdWho: {}
  pdEveryone: {}
  pdEpisode: { type: string }
  pdRunsOn: {}
  pdNoCareWorkflow: {}
  pdScopes: {}
  pdScopesNone: {}
  pdScope: { scope: string; version: string }
  pdOpenText: {}
  pdOverrides: { count: number }
  pdBoard: {}
  pdBoardHint: {}
  pdNoBoard: {}
  pdStage: { n: number; id: string }
  pdFinal: {}
  pdForm: {}
  pdHappens: {}
  pdNothingHappens: {}
  pdInside: { states: string }
  pdTask: { role: string; worklist: string }
  pdGate: { gate: string }
  pdInvoke: { src: string }
  pdExecute: { action: string }
  pdRaise: { event: string }
  pdAssign: { field: string; value: string }
  pdInvestigations: {}
  pdRegimens: { count: number }
  pdRegimensHint: {}
  pdRule: { n: number; title: string }
  pdRuleWhen: { condition: string }
  pdRuleNoCondition: {}
  pdRuleSource: { source: string }
  pdRuleUncited: {}
  pdRuleNoActivity: {}
  pdMoveUp: { title: string }
  pdMoveDown: { title: string }
  pdMoved: {}
  pdSchedule: {}
  pdTimer: { event: string; after: string }
  pdOutcomes: {}
  pdIndicators: {}
  pdLeaves: {}
  pdLeavesFinal: {}
  pdLeavesNever: {}
  pdExit: { targets: string; events: string }
  pdExitTimer: { targets: string; after: string }
  pdExitEventless: { targets: string }
  pdGuard: { guard: string }
  pdKind: { kind: string }
  pdWhere: { where: string; version: string }
  pdOpenArtefact: { canonical: string }
  pdCreate: {}
  pdCreateLabel: { canonical: string }
  pdCreateFailed: { reason: string }
  pdReadOnly: { reason: string }
  pdStale: {}
  pdReload: {}
  pdSaveFailed: { reason: string }
  pdPanel: {}
  pdPanelHint: {}
  pdMissingFile: {}
  pdStillMissing: { count: number }
  pdNothingMissing: {}
  pdGapMissing: { kind: string; canonical: string }
  pdGapNoCareWorkflow: {}
  pdGapUnknownStage: { stage: string }
  pdGapUncited: { stage: string; title: string }
  pdGapNoCondition: { stage: string; title: string }
  pdGapNoActivity: { stage: string; title: string }
  pdGapPinned: { kind: string; canonical: string; version: string }
  pdAuthority: {}
}

export const programMessages = defineMessages<ProgramValues>({
  pdOpen: { id: 'app.studio.program.open', defaultMessage: '{path} in the Program Designer', description: 'Program Designer: link from a program PlanDefinition to the Program Designer' },
  pdOpenShort: { id: 'app.studio.program.openShort', defaultMessage: 'Program Designer', description: 'Program Designer: visible text of the link to it' },
  pdHeading: { id: 'app.studio.program.heading', defaultMessage: 'Program Designer: {title}', description: 'Program Designer: heading' },
  pdLoading: { id: 'app.studio.program.loading', defaultMessage: 'Loading the program…', description: 'Program Designer: loading' },
  pdRefused: { id: 'app.studio.program.refused', defaultMessage: '{refusal, select, not_json {This file is not JSON.} other {This file is not a program: a PlanDefinition of type clinical-protocol.}}', description: 'Program Designer: the file cannot be opened' },
  pdFile: { id: 'app.studio.program.file', defaultMessage: '{path} · {url}|{version}', description: 'Program Designer: the file and canonical being designed' },
  pdStrip: { id: 'app.studio.program.strip', defaultMessage: 'The whole program', description: 'Program Designer: label of the strip across the top' },
  pdWho: { id: 'app.studio.program.who', defaultMessage: 'Who it is for', description: 'Program Designer: the eligibility FHIRPath' },
  pdEveryone: { id: 'app.studio.program.everyone', defaultMessage: 'Everyone: no eligibility rule', description: 'Program Designer: a program without eligibility' },
  pdEpisode: { id: 'app.studio.program.episode', defaultMessage: 'Enrolment opens an EpisodeOfCare of type {type}', description: 'Program Designer: the episode type' },
  pdRunsOn: { id: 'app.studio.program.runsOn', defaultMessage: 'Runs on', description: 'Program Designer: the care workflow' },
  pdNoCareWorkflow: { id: 'app.studio.program.noCareWorkflow', defaultMessage: 'No care workflow: the plan names no Workflow Definition.', description: 'Program Designer: no care workflow' },
  pdScopes: { id: 'app.studio.program.scopes', defaultMessage: 'Activated for', description: 'Program Designer: the scopes' },
  pdScopesNone: { id: 'app.studio.program.scopesNone', defaultMessage: 'Not activated anywhere yet', description: 'Program Designer: no scopes' },
  pdScope: { id: 'app.studio.program.scope', defaultMessage: '{scope} ({version})', description: 'Program Designer: a scope and the version active there' },
  pdOpenText: { id: 'app.studio.program.openText', defaultMessage: 'Open', description: 'Program Designer: visible text of a link to an editor' },
  pdOverrides: { id: 'app.studio.program.overrides', defaultMessage: '{count, plural, =0 {No override points: a facility admin may change nothing} one {# override point a facility admin may fill in} other {# override points a facility admin may fill in}}', description: 'Program Designer: the number of override points' },
  pdBoard: { id: 'app.studio.program.board', defaultMessage: 'Journey', description: 'Program Designer: the journey board' },
  pdBoardHint: { id: 'app.studio.program.boardHint', defaultMessage: 'One column per state of the care workflow. Add or reorder stages in the Workflow Studio.', description: 'Program Designer: what the board shows' },
  pdNoBoard: { id: 'app.studio.program.noBoard', defaultMessage: 'The journey shows once the care workflow exists and can be read.', description: 'Program Designer: no board' },
  pdStage: { id: 'app.studio.program.stage', defaultMessage: '{n}. {id}', description: 'Program Designer: a stage heading' },
  pdFinal: { id: 'app.studio.program.final', defaultMessage: 'final', description: 'Program Designer: a final stage' },
  pdForm: { id: 'app.studio.program.form', defaultMessage: 'Form', description: 'Program Designer: a stage form' },
  pdHappens: { id: 'app.studio.program.happens', defaultMessage: 'What happens', description: 'Program Designer: a stage’s actions' },
  pdNothingHappens: { id: 'app.studio.program.nothingHappens', defaultMessage: 'Nothing: the patient waits here', description: 'Program Designer: a stage with no actions' },
  pdInside: { id: 'app.studio.program.inside', defaultMessage: 'Inside: {states}', description: 'Program Designer: the states inside a compound stage' },
  pdTask: { id: 'app.studio.program.task', defaultMessage: 'A task for the {role} on the {worklist} worklist', description: 'Program Designer: a human task' },
  pdGate: { id: 'app.studio.program.gate', defaultMessage: 'Waits for payment at {gate}', description: 'Program Designer: a payment gate' },
  pdInvoke: { id: 'app.studio.program.invoke', defaultMessage: 'Runs the workflow {src}', description: 'Program Designer: a sub-workflow' },
  pdExecute: { id: 'app.studio.program.execute', defaultMessage: 'Runs {action}', description: 'Program Designer: a typed action' },
  pdRaise: { id: 'app.studio.program.raise', defaultMessage: 'Raises {event}', description: 'Program Designer: a raise action' },
  pdAssign: { id: 'app.studio.program.assign', defaultMessage: 'Sets {field} to {value}', description: 'Program Designer: an assign action' },
  pdInvestigations: { id: 'app.studio.program.investigations', defaultMessage: 'Investigations', description: 'Program Designer: heading' },
  pdRegimens: { id: 'app.studio.program.regimens', defaultMessage: 'Regimen templates ({count})', description: 'Program Designer: heading' },
  pdRegimensHint: { id: 'app.studio.program.regimensHint', defaultMessage: 'The first rule that matches the patient chooses the regimen, from the top down.', description: 'Program Designer: how regimen rules are taken' },
  pdRule: { id: 'app.studio.program.rule', defaultMessage: '{n}. {title}', description: 'Program Designer: a regimen rule' },
  pdRuleWhen: { id: 'app.studio.program.ruleWhen', defaultMessage: 'When {condition}', description: 'Program Designer: a regimen rule’s condition' },
  pdRuleNoCondition: { id: 'app.studio.program.ruleNoCondition', defaultMessage: 'No condition: it would match every patient', description: 'Program Designer: a rule without a condition' },
  pdRuleSource: { id: 'app.studio.program.ruleSource', defaultMessage: 'Cites {source}', description: 'Program Designer: the protocol a rule cites' },
  pdRuleUncited: { id: 'app.studio.program.ruleUncited', defaultMessage: 'Cites no approved protocol source', description: 'Program Designer: a rule citing no approved source' },
  pdRuleNoActivity: { id: 'app.studio.program.ruleNoActivity', defaultMessage: 'Uses no ActivityDefinition', description: 'Program Designer: a rule without an ActivityDefinition' },
  pdMoveUp: { id: 'app.studio.program.moveUp', defaultMessage: 'Move “{title}” up', description: 'Program Designer: button' },
  pdMoveDown: { id: 'app.studio.program.moveDown', defaultMessage: 'Move “{title}” down', description: 'Program Designer: button' },
  pdMoved: { id: 'app.studio.program.moved', defaultMessage: 'The order is saved.', description: 'Program Designer: a reorder saved' },
  pdSchedule: { id: 'app.studio.program.schedule', defaultMessage: 'Schedule', description: 'Program Designer: heading' },
  pdTimer: { id: 'app.studio.program.timer', defaultMessage: '{event} after {after}', description: 'Program Designer: a timer' },
  pdOutcomes: { id: 'app.studio.program.outcomes', defaultMessage: 'Outcomes', description: 'Program Designer: heading' },
  pdIndicators: { id: 'app.studio.program.indicators', defaultMessage: 'Indicators', description: 'Program Designer: heading' },
  pdLeaves: { id: 'app.studio.program.leaves', defaultMessage: 'Leaves when', description: 'Program Designer: heading' },
  pdLeavesFinal: { id: 'app.studio.program.leavesFinal', defaultMessage: 'Never: the journey ends here', description: 'Program Designer: a final stage' },
  pdLeavesNever: { id: 'app.studio.program.leavesNever', defaultMessage: 'No way out', description: 'Program Designer: a stage with no transitions' },
  pdExit: { id: 'app.studio.program.exit', defaultMessage: '{events} → {targets}', description: 'Program Designer: an exit on events' },
  pdExitTimer: { id: 'app.studio.program.exitTimer', defaultMessage: 'After {after} → {targets}', description: 'Program Designer: an exit on a timer' },
  pdExitEventless: { id: 'app.studio.program.exitEventless', defaultMessage: 'As soon as it holds → {targets}', description: 'Program Designer: an eventless exit' },
  pdGuard: { id: 'app.studio.program.guard', defaultMessage: 'if {guard}', description: 'Program Designer: an exit’s guard' },
  pdKind: { id: 'app.studio.program.kind', defaultMessage: '{kind, select, Questionnaire {Form} Library {Workflow} ActivityDefinition {ActivityDefinition} Measure {Measure} other {{kind}}}', description: 'Program Designer: an artefact’s kind' },
  pdWhere: { id: 'app.studio.program.where', defaultMessage: '{where, select, draft {in this draft, {version}} published {published, pinned to {version}} other {not created yet}}', description: 'Program Designer: where a linked artefact stands' },
  pdOpenArtefact: { id: 'app.studio.program.openArtefact', defaultMessage: 'Open {canonical}', description: 'Program Designer: open an artefact in its editor' },
  pdCreate: { id: 'app.studio.program.create', defaultMessage: 'Create', description: 'Program Designer: button' },
  pdCreateLabel: { id: 'app.studio.program.createLabel', defaultMessage: 'Create {canonical}', description: 'Program Designer: accessible name of Create' },
  pdCreateFailed: { id: 'app.studio.program.createFailed', defaultMessage: 'Not created. {reason}', description: 'Program Designer: a create refused' },
  pdReadOnly: { id: 'app.studio.program.readOnly', defaultMessage: 'Read only: {reason}', description: 'Program Designer: the person may not edit' },
  pdStale: { id: 'app.studio.program.stale', defaultMessage: 'Someone saved this draft since you opened it, so nothing was kept. Reload to see their change.', description: 'Program Designer: 412' },
  pdReload: { id: 'app.studio.program.reload', defaultMessage: 'Reload', description: 'Program Designer: button' },
  pdSaveFailed: { id: 'app.studio.program.saveFailed', defaultMessage: 'The order was not saved. {reason}', description: 'Program Designer: a reorder refused' },
  pdPanel: { id: 'app.studio.program.panel', defaultMessage: 'Package', description: 'Program Designer: the package panel' },
  pdPanelHint: { id: 'app.studio.program.panelHint', defaultMessage: 'Grouped by kind; the package keeps its artefacts side by side.', description: 'Program Designer: how files are grouped' },
  pdMissingFile: { id: 'app.studio.program.missingFile', defaultMessage: 'missing', description: 'Program Designer: a missing file in the package panel' },
  pdStillMissing: { id: 'app.studio.program.stillMissing', defaultMessage: 'Still missing ({count})', description: 'Program Designer: heading' },
  pdNothingMissing: { id: 'app.studio.program.nothingMissing', defaultMessage: 'Nothing missing.', description: 'Program Designer: no gaps' },
  pdGapMissing: { id: 'app.studio.program.gapMissing', defaultMessage: '{kind, select, Questionnaire {Form} Library {Workflow} other {{kind}}} {canonical} does not exist yet', description: 'Program Designer: a missing artefact' },
  pdGapNoCareWorkflow: { id: 'app.studio.program.gapNoCareWorkflow', defaultMessage: 'The plan names no care workflow', description: 'Program Designer: gap' },
  pdGapUnknownStage: { id: 'app.studio.program.gapUnknownStage', defaultMessage: 'The plan has investigations or regimens at {stage}, a state the care workflow does not have', description: 'Program Designer: gap' },
  pdGapUncited: { id: 'app.studio.program.gapUncited', defaultMessage: 'The regimen template “{title}” at {stage} cites no approved protocol source', description: 'Program Designer: gap' },
  pdGapNoCondition: { id: 'app.studio.program.gapNoCondition', defaultMessage: 'The regimen template “{title}” at {stage} has no FHIRPath condition', description: 'Program Designer: gap' },
  pdGapNoActivity: { id: 'app.studio.program.gapNoActivity', defaultMessage: 'The regimen template “{title}” at {stage} uses no ActivityDefinition', description: 'Program Designer: gap' },
  pdGapPinned: { id: 'app.studio.program.gapPinned', defaultMessage: '{kind, select, Questionnaire {Form} Library {Workflow} other {{kind}}} {canonical} is pinned, but the package holds {version}', description: 'Program Designer: gap' },
  pdAuthority: { id: 'app.studio.program.authority', defaultMessage: 'Check now on the draft page remains the authority.', description: 'Program Designer: the gaps are not the publication checks' },
})

// The Studio home and its sections (#855; studio.md § The Studio home): what a person designs,
// with the packages behind the editors.
type DesignValues = {
  dhIntro: {}
  dhLoading: {}
  dhSection: { section: string }
  dhSectionHint: { section: string }
  dhCount: { section: string; count: number }
  dhOpen: { section: string }
  dhNew: { section: string }
  dhPackages: {}
  dhPackagesHint: {}
  dhPackagesCount: { count: number }
  dhPackagesOpen: {}
  dhHome: {}
  dhColTitle: {}
  dhColType: {}
  dhColPackage: {}
  dhColStatus: {}
  dhColChanged: {}
  dhKind: { kind: string }
  dhStatus: { status: string; holder: string }
  dhStatusAny: {}
  dhSomeone: {}
  dhPackageAll: {}
  dhSearch: {}
  dhFilterPackage: {}
  dhFilterStatus: {}
  dhEmpty: { section: string }
  dhNoMatch: {}
  dhTableHint: {}
  dhNever: {}
  dhNewHeading: { section: string }
  dhNewPackage: {}
  dhNewInDraft: { title: string; id: string }
  dhNewNextVersion: { title: string; id: string }
  dhNewBlank: {}
  dhNewBlankName: {}
  dhNewBlankTitle: {}
  dhNewBlankKind: {}
  dhNewBlankCanonical: {}
  dhNewWhat: {}
  dhNewWorkflow: {}
  dhNewNoWorkflow: {}
  dhNewTitle: {}
  dhNewCreate: {}
  dhNewCancel: {}
  dhNewFailed: { reason: string }
  dhNewNoPackage: {}
  dhBarLabel: {}
  dhBarPackage: { title: string; name: string; version: string; stage: string }
  dhBarDraftPage: {}
  dhBarPackagePage: {}
  dhBarEdit: {}
  dhBarEditStart: { name: string }
  dhBarEditReuse: { id: string }
  dhBarHeld: { holder: string }
  dhBarSubmitted: {}
  dhBarEditing: {}
  dhBarEditFailed: { reason: string }
}

export const designMessages = defineMessages<DesignValues>({
  dhIntro: { id: 'app.studio.home.intro', defaultMessage: 'Design forms, workflows, concepts and decision rules. Each design lives in a package; drafts, review and publishing are handled behind the editors.', description: 'Studio home: what it is for' },
  dhLoading: { id: 'app.studio.home.loading', defaultMessage: 'Loading your designs…', description: 'Studio home: loading' },
  dhSection: {
    id: 'app.studio.home.section',
    defaultMessage: '{section, select, forms {Forms} workflows {Workflows} concepts {Concepts & value sets} decisionTables {Decision tables} programs {Programs} ageBands {Age band sets} indicators {Indicators} reports {Reports} dashboards {Dashboards} population {Population estimates} other {{section}}}',
    description: 'Studio: a section heading',
  },
  dhSectionHint: {
    id: 'app.studio.home.sectionHint',
    defaultMessage:
      '{section, select, forms {Questionnaires health workers fill in, designed with a live preview.} workflows {Care pathways as statecharts: states, transitions, timers and tasks, with a sandbox to play them.} concepts {Local code systems and the value sets forms bind to.} decisionTables {Rules that choose an outcome for a patient, used by a workflow’s guards.} programs {Care programs assembled from their workflow, forms and regimens.} ageBands {Named age ranges indicators and reports split their figures by, such as MoH’s ANC age bands.} indicators {Counts, percentages and rates over care, tried on the package’s test patients.} reports {HMIS forms and facility reports: indicators laid out for one period type, by section, with the levels they run for.} dashboards {Standard dashboards: charts, tables and single numbers of indicators over the last few periods, shown to every user over their own area.} population {A year’s population figures by place, imported from the HMIS lead’s CSV: the denominators of coverage indicators.} other {}}',
    description: 'Studio: what a section holds',
  },
  dhCount: {
    id: 'app.studio.home.count',
    defaultMessage:
      '{section, select, forms {{count, plural, =0 {No forms} one {# form} other {# forms}}} workflows {{count, plural, =0 {No workflows} one {# workflow} other {# workflows}}} concepts {{count, plural, =0 {No code systems or value sets} one {# code system or value set} other {# code systems and value sets}}} decisionTables {{count, plural, =0 {No decision tables} one {# decision table} other {# decision tables}}} programs {{count, plural, =0 {No programs} one {# program} other {# programs}}} ageBands {{count, plural, =0 {No age band sets} one {# age band set} other {# age band sets}}} indicators {{count, plural, =0 {No indicators} one {# indicator} other {# indicators}}} reports {{count, plural, =0 {No reports} one {# report} other {# reports}}} dashboards {{count, plural, =0 {No dashboards} one {# dashboard} other {# dashboards}}} population {{count, plural, =0 {No population estimates} one {# year of population estimates} other {# years of population estimates}}} other {{count}}}',
    description: 'Studio: how many designs a section holds',
  },
  dhOpen: {
    id: 'app.studio.home.open',
    defaultMessage: '{section, select, forms {Open forms} workflows {Open workflows} concepts {Open concepts & value sets} decisionTables {Open decision tables} programs {Open programs} ageBands {Open age band sets} indicators {Open indicators} reports {Open reports} dashboards {Open dashboards} population {Open population estimates} other {Open}}',
    description: 'Studio home: open a section',
  },
  dhNew: {
    id: 'app.studio.home.new',
    defaultMessage: '{section, select, forms {New form} workflows {New workflow} concepts {New code system or value set} decisionTables {New decision table} programs {New program} ageBands {New age band set} indicators {New indicator} reports {New report} dashboards {New dashboard} population {New year of estimates} other {New}}',
    description: 'Studio: start a new design',
  },
  dhPackages: { id: 'app.studio.home.packages', defaultMessage: 'Packages', description: 'Studio: the packages section heading' },
  dhPackagesHint: { id: 'app.studio.home.packagesHint', defaultMessage: 'The packages your designs live in: check, submit, review, approve, publish and activate them.', description: 'Studio home: the packages section' },
  dhPackagesCount: { id: 'app.studio.home.packagesCount', defaultMessage: '{count, plural, =0 {No packages} one {# package} other {# packages}}', description: 'Studio home: how many packages' },
  dhPackagesOpen: { id: 'app.studio.home.packagesOpen', defaultMessage: 'Open the packages', description: 'Studio home: open the package list' },
  dhHome: { id: 'app.studio.home.back', defaultMessage: 'Back to the Studio', description: 'Studio: back to the home' },
  dhColTitle: { id: 'app.studio.design.colTitle', defaultMessage: 'Title', description: 'Studio section: column' },
  dhColType: { id: 'app.studio.design.colType', defaultMessage: 'Type', description: 'Studio section: column' },
  dhColPackage: { id: 'app.studio.design.colPackage', defaultMessage: 'Package', description: 'Studio section: column' },
  dhColStatus: { id: 'app.studio.design.colStatus', defaultMessage: 'Status', description: 'Studio section: column' },
  dhColChanged: { id: 'app.studio.design.colChanged', defaultMessage: 'Last change', description: 'Studio section: column' },
  dhKind: {
    id: 'app.studio.design.kind',
    defaultMessage: '{kind, select, form {Form} workflow {Workflow} decisionTable {Decision table} codeSystem {Code system} valueSet {Value set} program {Program} ageBandSet {Age band set} indicator {Indicator} report {Report} dashboard {Dashboard} population {Population estimates} other {{kind}}}',
    description: 'Studio section: a design’s type',
  },
  dhStatus: {
    id: 'app.studio.design.status',
    defaultMessage: '{status, select, published {Published} yours {In your draft} theirs {In {holder}’s draft} submitted {Submitted for review} other {{status}}}',
    description: 'Studio section: where a design stands',
  },
  dhSomeone: { id: 'app.studio.design.someone', defaultMessage: 'someone else', description: 'Studio section: status filter, a draft held by another person' },
  dhStatusAny: { id: 'app.studio.design.statusAny', defaultMessage: 'Any status', description: 'Studio section: status filter' },
  dhPackageAll: { id: 'app.studio.design.packageAll', defaultMessage: 'All packages', description: 'Studio section: package filter' },
  dhSearch: { id: 'app.studio.design.search', defaultMessage: 'Search', description: 'Studio section: search field' },
  dhFilterPackage: { id: 'app.studio.design.filterPackage', defaultMessage: 'Package', description: 'Studio section: package filter' },
  dhFilterStatus: { id: 'app.studio.design.filterStatus', defaultMessage: 'Status', description: 'Studio section: status filter' },
  dhEmpty: {
    id: 'app.studio.design.empty',
    defaultMessage: '{section, select, forms {No forms yet.} workflows {No workflows yet.} concepts {No code systems or value sets yet.} decisionTables {No decision tables yet.} programs {No programs yet.} ageBands {No age band sets yet.} indicators {No indicators yet.} reports {No reports yet.} dashboards {No dashboards yet.} population {No population estimates yet.} other {Nothing yet.}}',
    description: 'Studio section: nothing designed yet',
  },
  dhNoMatch: { id: 'app.studio.design.noMatch', defaultMessage: 'Nothing matches the search and filters.', description: 'Studio section: filters hide everything' },
  dhTableHint: { id: 'app.studio.design.tableHint', defaultMessage: 'Click a row to open it in its editor, or move with the arrow keys and press Enter.', description: 'Studio section: how to use the table' },
  dhNever: { id: 'app.studio.design.never', defaultMessage: '—', description: 'Studio section: no last change known' },
  dhNewHeading: {
    id: 'app.studio.design.newHeading',
    defaultMessage: '{section, select, forms {A new form} workflows {A new workflow} concepts {A new code system or value set} decisionTables {A new decision table} programs {A new program} ageBands {A new age band set} indicators {A new indicator} reports {A new report} dashboards {A new dashboard} population {A new year of population estimates} other {New}}',
    description: 'Studio section: the New panel heading',
  },
  dhNewPackage: { id: 'app.studio.design.newPackage', defaultMessage: 'Which package', description: 'Studio New: the package choice' },
  dhNewInDraft: { id: 'app.studio.design.newInDraft', defaultMessage: '{title}: your draft {id}', description: 'Studio New: a draft the person holds' },
  dhNewNextVersion: { id: 'app.studio.design.newNextVersion', defaultMessage: '{title}: start the next version of {id}', description: 'Studio New: a published package' },
  dhNewBlank: { id: 'app.studio.design.newBlank', defaultMessage: 'A new package', description: 'Studio New: a new package' },
  dhNewBlankName: { id: 'app.studio.design.newBlankName', defaultMessage: 'Package name (e.g. ug.moh.anc)', description: 'Studio New: new package name' },
  dhNewBlankTitle: { id: 'app.studio.design.newBlankTitle', defaultMessage: 'Package title', description: 'Studio New: new package title' },
  dhNewBlankKind: { id: 'app.studio.design.newBlankKind', defaultMessage: 'Package kind', description: 'Studio New: new package kind' },
  dhNewBlankCanonical: { id: 'app.studio.design.newBlankCanonical', defaultMessage: 'Canonical base URL (e.g. https://health.go.ug/fhir)', description: 'Studio New: new package canonical base' },
  dhNewWhat: { id: 'app.studio.design.newWhat', defaultMessage: 'What to create', description: 'Studio New: code system or value set' },
  dhNewWorkflow: { id: 'app.studio.design.newWorkflow', defaultMessage: 'In which workflow', description: 'Studio New: the workflow a decision table goes in' },
  dhNewNoWorkflow: { id: 'app.studio.design.newNoWorkflow', defaultMessage: 'A decision table lives in a workflow. Create a workflow first in a package you can edit.', description: 'Studio New: no workflow to put a table in' },
  dhNewTitle: { id: 'app.studio.design.newTitle', defaultMessage: 'Title', description: 'Studio New: the design’s title' },
  dhNewCreate: { id: 'app.studio.design.newCreate', defaultMessage: 'Create and open', description: 'Studio New: create the design and open its editor' },
  dhNewCancel: { id: 'app.studio.design.newCancel', defaultMessage: 'Cancel', description: 'Studio New: close the panel' },
  dhNewFailed: { id: 'app.studio.design.newFailed', defaultMessage: 'It was not created: {reason}', description: 'Studio New: failed' },
  dhNewNoPackage: { id: 'app.studio.design.newNoPackage', defaultMessage: 'Choose a package.', description: 'Studio New: no package chosen' },
  dhBarLabel: { id: 'app.studio.bar.label', defaultMessage: 'Where this design lives', description: 'Studio editor header: landmark name' },
  dhBarPackage: { id: 'app.studio.bar.package', defaultMessage: 'Package {title} ({name}), version {version}: {stage}', description: 'Studio editor header: the package and draft' },
  dhBarDraftPage: { id: 'app.studio.bar.draftPage', defaultMessage: 'Open the draft page', description: 'Studio editor header: link to check, submit, diff and discuss' },
  dhBarPackagePage: { id: 'app.studio.bar.packagePage', defaultMessage: 'Open the package page', description: 'Studio editor header: link to a published package' },
  dhBarEdit: { id: 'app.studio.bar.edit', defaultMessage: 'Edit', description: 'Studio editor header: start or reuse a draft to edit this design' },
  dhBarEditStart: { id: 'app.studio.bar.editStart', defaultMessage: 'Starts the next version of {name} as your draft.', description: 'Studio editor header: what Edit does' },
  dhBarEditReuse: { id: 'app.studio.bar.editReuse', defaultMessage: 'Opens it in your draft {id}.', description: 'Studio editor header: what Edit does' },
  dhBarHeld: { id: 'app.studio.bar.held', defaultMessage: '{holder} holds this package’s draft. Ask them to hand it over to edit it.', description: 'Studio editor header: someone else is editing' },
  dhBarSubmitted: { id: 'app.studio.bar.submitted', defaultMessage: 'This package’s draft is submitted. It can be edited again if changes are requested.', description: 'Studio editor header: the draft is past Draft' },
  dhBarEditing: { id: 'app.studio.bar.editing', defaultMessage: 'Opening a draft…', description: 'Studio editor header: Edit pending' },
  dhBarEditFailed: { id: 'app.studio.bar.editFailed', defaultMessage: 'No draft was started: {reason}', description: 'Studio editor header: Edit failed' },
})

// An Age Band Set's table (#551; analytics.md § Age Band Sets and Population Estimates): name,
// unit, the date age is taken at and the from–to rows, with gaps and overlaps flagged.
type AgeBandValues = {
  abLoading: {}
  abUnreadable: { reason: string }
  abTitle: {}
  abUnit: {}
  abUnitName: { unit: string }
  abAgeAt: {}
  abAgeAtName: { at: string }
  abBands: {}
  abFrom: {}
  abTo: {}
  abToHint: {}
  abBand: {}
  abRow: { row: number }
  abOpen: {}
  abAdd: {}
  abRemove: { row: number }
  abUp: { row: number }
  abDown: { row: number }
  abActions: {}
  abProblems: { count: number }
  abNoProblems: {}
  abNoTitle: {}
  abNoBands: {}
  abNoFrom: { row: number }
  abNegative: { row: number }
  abGoesDown: { row: number }
  abGap: { first: string; second: string; ages: string }
  abOverlap: { first: string; second: string; ages: string }
  abAge: { age: number; unit: string }
  abAges: { from: number; to: number; unit: string }
  abAgesOver: { from: number; unit: string }
  abVersion: { version: string; reason: string; published: string }
  abMajorPackage: {}
  abSave: {}
  abSaved: {}
  abStale: {}
  abReload: {}
  abFailed: { reason: string }
  abReadOnly: {}
  abAuthority: {}
}

export const ageBandMessages = defineMessages<AgeBandValues>({
  abLoading: { id: 'app.studio.ageBands.loading', defaultMessage: 'Loading the age band set…', description: 'Age band set editor: loading' },
  abUnreadable: { id: 'app.studio.ageBands.unreadable', defaultMessage: 'This file is not an age band set the Studio can edit ({reason}). Open it in the file editor instead.', description: 'Age band set editor: the file cannot be read' },
  abTitle: { id: 'app.studio.ageBands.title', defaultMessage: 'Name', description: 'Age band set editor: the set’s name' },
  abUnit: { id: 'app.studio.ageBands.unit', defaultMessage: 'Unit', description: 'Age band set editor: the unit ages are counted in' },
  abUnitName: { id: 'app.studio.ageBands.unitName', defaultMessage: '{unit, select, days {Days} weeks {Weeks} months {Months} years {Years} other {{unit}}}', description: 'Age band set editor: a unit' },
  abAgeAt: { id: 'app.studio.ageBands.ageAt', defaultMessage: 'Age is taken at', description: 'Age band set editor: the date age is taken at' },
  abAgeAtName: {
    id: 'app.studio.ageBands.ageAtName',
    defaultMessage: '{at, select, periodEnd {The end of the reporting period} periodStart {The start of the reporting period} periodDate {The date of the care counted} other {{at}}}',
    description: 'Age band set editor: a date age is taken at',
  },
  abBands: { id: 'app.studio.ageBands.bands', defaultMessage: 'Bands', description: 'Age band set editor: the table of bands' },
  abFrom: { id: 'app.studio.ageBands.from', defaultMessage: 'From', description: 'Age band set editor: column, the lowest age in a band' },
  abTo: { id: 'app.studio.ageBands.to', defaultMessage: 'To', description: 'Age band set editor: column, the highest age in a band' },
  abToHint: { id: 'app.studio.ageBands.toHint', defaultMessage: 'Both ages are included. Leave To empty for the last band to take every age above.', description: 'Age band set editor: how From and To work' },
  abBand: { id: 'app.studio.ageBands.band', defaultMessage: 'Band', description: 'Age band set editor: column, the band as read' },
  abRow: { id: 'app.studio.ageBands.row', defaultMessage: 'Band {row}', description: 'Age band set editor: a row' },
  abOpen: { id: 'app.studio.ageBands.open', defaultMessage: 'and over', description: 'Age band set editor: an empty To' },
  abAdd: { id: 'app.studio.ageBands.add', defaultMessage: 'Add a band', description: 'Age band set editor: add a row' },
  abRemove: { id: 'app.studio.ageBands.remove', defaultMessage: 'Remove band {row}', description: 'Age band set editor: remove a row' },
  abUp: { id: 'app.studio.ageBands.up', defaultMessage: 'Move band {row} up', description: 'Age band set editor: reorder' },
  abDown: { id: 'app.studio.ageBands.down', defaultMessage: 'Move band {row} down', description: 'Age band set editor: reorder' },
  abActions: { id: 'app.studio.ageBands.actions', defaultMessage: 'Actions', description: 'Age band set editor: column' },
  abProblems: { id: 'app.studio.ageBands.problems', defaultMessage: '{count, plural, one {# problem} other {# problems}}', description: 'Age band set editor: problems heading' },
  abNoProblems: { id: 'app.studio.ageBands.noProblems', defaultMessage: 'No gaps or overlaps.', description: 'Age band set editor: the bands are contiguous' },
  abNoTitle: { id: 'app.studio.ageBands.noTitle', defaultMessage: 'The set has no name.', description: 'Age band set editor: problem' },
  abNoBands: { id: 'app.studio.ageBands.noBands', defaultMessage: 'The set has no bands.', description: 'Age band set editor: problem' },
  abNoFrom: { id: 'app.studio.ageBands.noFrom', defaultMessage: 'Band {row} has no From age.', description: 'Age band set editor: problem' },
  abNegative: { id: 'app.studio.ageBands.negative', defaultMessage: 'Band {row} starts below zero.', description: 'Age band set editor: problem' },
  abGoesDown: { id: 'app.studio.ageBands.goesDown', defaultMessage: 'Band {row} ends below where it starts.', description: 'Age band set editor: problem' },
  abGap: { id: 'app.studio.ageBands.gap', defaultMessage: 'Gap between {first} and {second}: {ages} are in no band.', description: 'Age band set editor: problem' },
  abOverlap: { id: 'app.studio.ageBands.overlap', defaultMessage: 'Overlap between {first} and {second}: {ages} are in both.', description: 'Age band set editor: problem' },
  abAge: { id: 'app.studio.ageBands.age', defaultMessage: '{age} {unit}', description: 'Age band set editor: one age in a problem' },
  abAges: { id: 'app.studio.ageBands.ages', defaultMessage: '{from}–{to} {unit}', description: 'Age band set editor: a range of ages in a problem' },
  abAgesOver: { id: 'app.studio.ageBands.agesOver', defaultMessage: '{from} {unit} and over', description: 'Age band set editor: ages from one up, in a problem' },
  abVersion: {
    id: 'app.studio.ageBands.version',
    defaultMessage:
      '{reason, select, new {Version {version}: a first version.} unchanged {Version {version}: unchanged since it was published.} bands {Version {version}: the bands changed since {published}, so it is a major version. Indicators and reports pinned to {published} keep its bands.} renamed {Version {version}: only the name changed since {published}, a patch.} other {Version {version}}}',
    description: 'Age band set editor: the version the set takes and why',
  },
  abMajorPackage: { id: 'app.studio.ageBands.majorPackage', defaultMessage: 'The package needs a major version too: change it on the draft page before submitting.', description: 'Age band set editor: a band change needs a major package version' },
  abSave: { id: 'app.studio.ageBands.save', defaultMessage: 'Save', description: 'Age band set editor: save' },
  abSaved: { id: 'app.studio.ageBands.saved', defaultMessage: 'Saved.', description: 'Age band set editor: saved' },
  abStale: { id: 'app.studio.ageBands.stale', defaultMessage: 'Someone saved this draft since you opened it. Nothing was saved: reload to see their change.', description: 'Age band set editor: 412' },
  abReload: { id: 'app.studio.ageBands.reload', defaultMessage: 'Reload', description: 'Age band set editor: reload after a 412' },
  abFailed: { id: 'app.studio.ageBands.failed', defaultMessage: 'It was not saved: {reason}', description: 'Age band set editor: save failed' },
  abReadOnly: { id: 'app.studio.ageBands.readOnly', defaultMessage: 'Read only: use Edit above to change it in a draft.', description: 'Age band set editor: not editable' },
  abAuthority: { id: 'app.studio.ageBands.authority', defaultMessage: 'Check now on the draft page remains the authority.', description: 'Age band set editor: the flags are not the publication checks' },
})

// The indicator editor (#537; studio.md § Indicators): the definition, the condition trees and
// Try over the package's test patients.
type IndicatorValues = {
  inSaveTest: {}
  inTestSaved: { path: string }
  inTestUnsaved: {}
  inOpen: { path: string }
  inOpenShort: {}
  inLoading: {}
  inUnreadable: { reason: string }
  inReadOnly: {}
  inProblems: { count: number }
  inNoProblems: {}
  inAuthority: {}
  inNoTitle: {}
  inNoPeriodDate: {}
  inNoValue: {}
  inUnpinned: { measure: string }
  inItself: {}
  inUnknown: { measure: string }
  inCircle: { through: string }
  inNoDisaggregationName: { at: number }
  inTwice: { name: string }
  inDefinition: {}
  inTitle: {}
  inDescription: {}
  inCounts: {}
  inCountsName: { counts: string }
  inBasis: {}
  inPeriodDate: {}
  inPeriodDateHint: {}
  inDateName: { date: string }
  inCalculates: {}
  inCalculation: {}
  inCalculationName: { calculation: string }
  inRatePer: {}
  inPick: {}
  inPickName: { pick: string }
  inCumulative: {}
  inValue: {}
  inValueField: {}
  inValueBetween: {}
  inValueExpression: {}
  inValueFrom: {}
  inValueTo: {}
  inValueUnit: {}
  inUnitName: { unit: string }
  inNumerator: {}
  inNumeratorHint: {}
  inCounted: {}
  inCountedHint: {}
  inDenominator: {}
  inDenominatorHint: {}
  inAddDenominator: {}
  inRemoveDenominator: {}
  inDenominatorFrom: {}
  inFromRecords: {}
  inFromEstimate: { title: string }
  inEstimateHint: {}
  inNoEstimateTypes: {}
  inCompiled: {}
  inCompiledOf: { side: string }
  inGroup: { at: string; join: string }
  inJoin: { at: string }
  inJoinName: { join: string }
  inEmptyGroup: {}
  inAddCondition: { at: string }
  inAddDate: { at: string }
  inAddMember: { at: string }
  inAddGroup: { at: string }
  inAddRaw: { at: string }
  inRemove: { at: string }
  inRaw: { at: string }
  inInput: { at: string }
  inOperator: { at: string }
  inKind: { at: string }
  inKindName: { kind: string }
  inLiteral: { at: string }
  inMemberIsIn: {}
  inMemberOf: {}
  inMemberSide: { at: string }
  inMemberMeasure: { at: string }
  inMemberChoose: {}
  inMemberOption: { title: string; canonical: string }
  inSideName: { side: string }
  inDateInput: { at: string }
  inDateOp: { at: string }
  inDateOpName: { op: string }
  inAnchor: { at: string; name: string }
  inAnchorDate: { at: string; name: string }
  inAnchorName: { anchor: string }
  inFrom: {}
  inTo: {}
  inAnd: {}
  inAmount: { at: string }
  inUnit: { at: string }
  inDirection: { at: string }
  inDirectionName: { direction: string }
  inPeriods: { at: string }
  inPeriodsWord: {}
  inDisaggregations: {}
  inDisaggregationsHint: {}
  inNoDisaggregations: {}
  inByAge: {}
  inByCode: {}
  inAgeSet: { at: number }
  inCodedName: { at: number }
  inCodedCriteria: { at: number }
  inRemoveDisaggregation: { at: number }
  inAddAge: {}
  inAddCoded: {}
  inSave: {}
  inSaved: {}
  inStale: {}
  inReload: {}
  inFailed: { reason: string }
  inTry: {}
  inTryHint: {}
  inPeriodType: {}
  inPeriodTypeName: { type: string }
  inPeriod: {}
  inPeriodExample: { type: string }
  inTrying: {}
  inTryFailed: { reason: string }
  inPeriodRange: { id: string; start: string; end: string }
  inFigure: {}
  inValueShown: { value: string; calculation: string; per: number }
  inParts: { count: number; denominator: number }
  inCountShown: { count: number }
  inValuesShown: { count: number; sum: string; minimum: string; maximum: string }
  inByTitle: { title: string }
  inBandColumn: {}
  inNumeratorColumn: {}
  inCountColumn: {}
  inDenominatorColumn: {}
  inValueColumn: {}
  inErrors: {}
  inPatients: { counted: number; total: number }
  inPatientLine: { name: string; counted: string; count: number; denominator: string }
  inReason: { reason: string }
  inRecordValue: { value: string; picked: string }
  inExplainSide: { side: string }
  inHeld: { held: string }
  inMapping: {}
  inMappingHint: {}
  inMappingNone: {}
  inMappingRow: { title: string; section: string; label: string }
  inMappingWhere: { package: string; published: string; pinned: string; sent: string }
}

export const indicatorMessages = defineMessages<IndicatorValues>({
  inSaveTest: { id: 'app.studio.indicator.saveTest', defaultMessage: 'Save as test', description: 'Indicator editor: keep what Try gave in tests/' },
  inTestSaved: { id: 'app.studio.indicator.testSaved', defaultMessage: 'Saved as {path}. Submission fails if a later edit changes these figures.', description: 'Indicator editor: the test was saved' },
  inTestUnsaved: { id: 'app.studio.indicator.testUnsaved', defaultMessage: 'Save the indicator first: a test keeps what the saved indicator gives.', description: 'Indicator editor: why Save as test is off' },
  inOpen: { id: 'app.studio.indicator.open', defaultMessage: 'Open in the indicator editor: {path}', description: 'Draft page: open a Measure' },
  inOpenShort: { id: 'app.studio.indicator.openShort', defaultMessage: 'Open in the indicator editor', description: 'Draft page: open a Measure' },
  inLoading: { id: 'app.studio.indicator.loading', defaultMessage: 'Loading the indicator…', description: 'Indicator editor' },
  inUnreadable: { id: 'app.studio.indicator.unreadable', defaultMessage: 'This file is not an indicator the Studio can edit ({reason}). Open it in the file editor instead.', description: 'Indicator editor' },
  inReadOnly: { id: 'app.studio.indicator.readOnly', defaultMessage: 'Read only. Press Edit to change it in your draft.', description: 'Indicator editor' },
  inProblems: { id: 'app.studio.indicator.problems', defaultMessage: '{count, plural, one {# problem} other {# problems}}', description: 'Indicator editor' },
  inNoProblems: { id: 'app.studio.indicator.noProblems', defaultMessage: 'No problems found while editing.', description: 'Indicator editor' },
  inAuthority: { id: 'app.studio.indicator.authority', defaultMessage: 'Check now on the draft page remains the authority: Validation refuses the same problems.', description: 'Indicator editor' },
  inNoTitle: { id: 'app.studio.indicator.noTitle', defaultMessage: 'The indicator has no name.', description: 'Indicator editor' },
  inNoPeriodDate: { id: 'app.studio.indicator.noPeriodDate', defaultMessage: 'The indicator has no period date: choose the date that places care in a Report’s period.', description: 'Indicator editor' },
  inNoValue: { id: 'app.studio.indicator.noValue', defaultMessage: 'The indicator takes a value but says none.', description: 'Indicator editor' },
  inUnpinned: { id: 'app.studio.indicator.unpinned', defaultMessage: '{measure} is not pinned to a version.', description: 'Indicator editor' },
  inItself: { id: 'app.studio.indicator.itself', defaultMessage: 'The indicator refers to itself.', description: 'Indicator editor' },
  inUnknown: { id: 'app.studio.indicator.unknown', defaultMessage: '{measure} is not an indicator of this package.', description: 'Indicator editor' },
  inCircle: { id: 'app.studio.indicator.circle', defaultMessage: 'The indicators refer to each other in a circle: this one → {through} → this one.', description: 'Indicator editor' },
  inNoDisaggregationName: { id: 'app.studio.indicator.noDisaggregationName', defaultMessage: 'Disaggregation {at} has no name.', description: 'Indicator editor' },
  inTwice: { id: 'app.studio.indicator.twice', defaultMessage: 'The indicator is disaggregated by {name} twice.', description: 'Indicator editor' },
  inDefinition: { id: 'app.studio.indicator.definition', defaultMessage: 'Definition', description: 'Indicator editor' },
  inTitle: { id: 'app.studio.indicator.title', defaultMessage: 'Name', description: 'Indicator editor' },
  inDescription: { id: 'app.studio.indicator.description', defaultMessage: 'Description', description: 'Indicator editor' },
  inCounts: { id: 'app.studio.indicator.counts', defaultMessage: 'What is counted', description: 'Indicator editor' },
  inCountsName: { id: 'app.studio.indicator.countsName', defaultMessage: '{counts, select, people {People, each once} enrollments {Enrollments, each once} visits {Visits} observations {Observations} other {{counts}}}', description: 'Indicator editor' },
  inBasis: { id: 'app.studio.indicator.basis', defaultMessage: 'Records it reads', description: 'Indicator editor' },
  inPeriodDate: { id: 'app.studio.indicator.periodDate', defaultMessage: 'Period date', description: 'Indicator editor' },
  inPeriodDateHint: { id: 'app.studio.indicator.periodDateHint', defaultMessage: 'The declared date that places care in a Report’s period. Required.', description: 'Indicator editor' },
  inDateName: { id: 'app.studio.indicator.dateName', defaultMessage: '{date, select, visitStart {When the visit started} visitEnd {When the visit ended} observed {When it was observed} enrollmentStart {When the enrollment started} enrollmentEnd {When the enrollment ended} birthDate {Date of birth} nextAppointment {Next booked appointment} lastMenstrualPeriod {Last menstrual period} other {{date}}}', description: 'Indicator editor' },
  inCalculates: { id: 'app.studio.indicator.calculates', defaultMessage: 'What it calculates', description: 'Indicator editor' },
  inCalculation: { id: 'app.studio.indicator.calculation', defaultMessage: 'Calculation', description: 'Indicator editor' },
  inCalculationName: { id: 'app.studio.indicator.calculationName', defaultMessage: '{calculation, select, count {Count} percentage {Percentage (× 100)} rate {Rate} ratio {Ratio} sum {Sum of a value} average {Average of a value} median {Median of a value} minimum {Minimum of a value} maximum {Maximum of a value} distinctCount {Count of distinct values} other {{calculation}}}', description: 'Indicator editor' },
  inRatePer: { id: 'app.studio.indicator.ratePer', defaultMessage: 'Per', description: 'Indicator editor' },
  inPick: { id: 'app.studio.indicator.pick', defaultMessage: 'Each one’s value', description: 'Indicator editor' },
  inPickName: { id: 'app.studio.indicator.pickName', defaultMessage: '{pick, select, last {The last in the period} other {The first in the period}}', description: 'Indicator editor' },
  inCumulative: { id: 'app.studio.indicator.cumulative', defaultMessage: 'Cumulative: count from the first record up to the end of the period', description: 'Indicator editor' },
  inValue: { id: 'app.studio.indicator.value', defaultMessage: 'The value', description: 'Indicator editor' },
  inValueField: { id: 'app.studio.indicator.valueField', defaultMessage: 'A field', description: 'Indicator editor' },
  inValueBetween: { id: 'app.studio.indicator.valueBetween', defaultMessage: 'Time between two dates', description: 'Indicator editor' },
  inValueExpression: { id: 'app.studio.indicator.valueExpression', defaultMessage: 'The value’s FHIRPath', description: 'Indicator editor' },
  inValueFrom: { id: 'app.studio.indicator.valueFrom', defaultMessage: 'From date', description: 'Indicator editor' },
  inValueTo: { id: 'app.studio.indicator.valueTo', defaultMessage: 'To date', description: 'Indicator editor' },
  inValueUnit: { id: 'app.studio.indicator.valueUnit', defaultMessage: 'In', description: 'Indicator editor' },
  inUnitName: { id: 'app.studio.indicator.unitName', defaultMessage: '{unit, select, days {days} weeks {weeks} months {months} years {years} other {{unit}}}', description: 'Indicator editor' },
  inNumerator: { id: 'app.studio.indicator.numerator', defaultMessage: 'Numerator', description: 'Indicator editor' },
  inNumeratorHint: { id: 'app.studio.indicator.numeratorHint', defaultMessage: 'Of those in the denominator, the ones counted.', description: 'Indicator editor' },
  inCounted: { id: 'app.studio.indicator.counted', defaultMessage: 'Counted', description: 'Indicator editor' },
  inCountedHint: { id: 'app.studio.indicator.countedHint', defaultMessage: 'The records with care in the period that are counted.', description: 'Indicator editor' },
  inDenominator: { id: 'app.studio.indicator.denominator', defaultMessage: 'Denominator', description: 'Indicator editor' },
  inDenominatorHint: { id: 'app.studio.indicator.denominatorHint', defaultMessage: 'The records with care in the period the numerator is a share of.', description: 'Indicator editor' },
  inAddDenominator: { id: 'app.studio.indicator.addDenominator', defaultMessage: '+ Add denominator', description: 'Indicator editor' },
  inRemoveDenominator: { id: 'app.studio.indicator.removeDenominator', defaultMessage: 'Remove denominator', description: 'Indicator editor' },
  inDenominatorFrom: { id: 'app.studio.indicator.denominatorFrom', defaultMessage: 'Denominator from', description: 'Indicator editor: where the denominator comes from (#864)' },
  inFromRecords: { id: 'app.studio.indicator.fromRecords', defaultMessage: 'Records: the conditions below', description: 'Indicator editor: a denominator counted from records' },
  inFromEstimate: { id: 'app.studio.indicator.fromEstimate', defaultMessage: '{title} (Population estimate)', description: 'Indicator editor: a denominator from a Population Estimate type (#553)' },
  inEstimateHint: {
    id: 'app.studio.indicator.estimateHint',
    defaultMessage: 'Each place’s estimate for the period, taken pro rata from the year’s Population package. A place without an estimate shows no figure. Try counts the numerator only.',
    description: 'Indicator editor: what an estimate denominator does (#864)',
  },
  inNoEstimateTypes: { id: 'app.studio.indicator.noEstimateTypes', defaultMessage: 'No Population package declares an estimate type yet.', description: 'Indicator editor: nothing to pick as an estimate denominator' },
  inCompiled: { id: 'app.studio.indicator.compiled', defaultMessage: 'Saved as FHIRPath:', description: 'Indicator editor' },
  inCompiledOf: { id: 'app.studio.indicator.compiledOf', defaultMessage: '{side} as FHIRPath', description: 'Indicator editor' },
  inGroup: { id: 'app.studio.indicator.group', defaultMessage: 'Group {at}: {join}', description: 'Indicator editor' },
  inJoin: { id: 'app.studio.indicator.join', defaultMessage: 'Group {at} joins its conditions by', description: 'Indicator editor' },
  inJoinName: { id: 'app.studio.indicator.joinName', defaultMessage: '{join, select, all {All of these} any {Any of these} not {None of these} other {{join}}}', description: 'Indicator editor' },
  inEmptyGroup: { id: 'app.studio.indicator.emptyGroup', defaultMessage: 'Everyone: no condition yet.', description: 'Indicator editor' },
  inAddCondition: { id: 'app.studio.indicator.addCondition', defaultMessage: '+ condition', description: 'Indicator editor' },
  inAddDate: { id: 'app.studio.indicator.addDate', defaultMessage: '+ date', description: 'Indicator editor' },
  inAddMember: { id: 'app.studio.indicator.addMember', defaultMessage: '+ in another indicator', description: 'Indicator editor' },
  inAddGroup: { id: 'app.studio.indicator.addGroup', defaultMessage: '+ group', description: 'Indicator editor' },
  inAddRaw: { id: 'app.studio.indicator.addRaw', defaultMessage: '+ FHIRPath', description: 'Indicator editor' },
  inRemove: { id: 'app.studio.indicator.remove', defaultMessage: 'Remove what is at {at}', description: 'Indicator editor' },
  inRaw: { id: 'app.studio.indicator.raw', defaultMessage: 'FHIRPath at {at}', description: 'Indicator editor' },
  inInput: { id: 'app.studio.indicator.input', defaultMessage: 'Field at {at}', description: 'Indicator editor' },
  inOperator: { id: 'app.studio.indicator.operator', defaultMessage: 'Comparison at {at}', description: 'Indicator editor' },
  inKind: { id: 'app.studio.indicator.kind', defaultMessage: 'Kind of value at {at}', description: 'Indicator editor' },
  inKindName: { id: 'app.studio.indicator.kindName', defaultMessage: '{kind, select, boolean {yes/no} number {number} code {code} other {{kind}}}', description: 'Indicator editor' },
  inLiteral: { id: 'app.studio.indicator.literal', defaultMessage: 'Value at {at}', description: 'Indicator editor' },
  inMemberIsIn: { id: 'app.studio.indicator.memberIsIn', defaultMessage: 'is in', description: 'Indicator editor' },
  inMemberOf: { id: 'app.studio.indicator.memberOf', defaultMessage: 'of', description: 'Indicator editor' },
  inMemberSide: { id: 'app.studio.indicator.memberSide', defaultMessage: 'Side at {at}', description: 'Indicator editor' },
  inMemberMeasure: { id: 'app.studio.indicator.memberMeasure', defaultMessage: 'Indicator at {at}', description: 'Indicator editor' },
  inMemberChoose: { id: 'app.studio.indicator.memberChoose', defaultMessage: 'Choose an indicator', description: 'Indicator editor' },
  inMemberOption: { id: 'app.studio.indicator.memberOption', defaultMessage: '{title} ({canonical})', description: 'Indicator editor' },
  inSideName: { id: 'app.studio.indicator.sideName', defaultMessage: '{side, select, numerator {the numerator} denominator {the denominator} other {{side}}}', description: 'Indicator editor' },
  inDateInput: { id: 'app.studio.indicator.dateInput', defaultMessage: 'Date at {at}', description: 'Indicator editor' },
  inDateOp: { id: 'app.studio.indicator.dateOp', defaultMessage: 'Date condition at {at}', description: 'Indicator editor' },
  inDateOpName: { id: 'app.studio.indicator.dateOpName', defaultMessage: '{op, select, inPeriod {is in the reporting period} on {is on} before {is before} after {is after} between {is between} within {is within} previous {is in the previous} empty {is empty} notEmpty {is not empty} other {{op}}}', description: 'Indicator editor' },
  inAnchor: { id: 'app.studio.indicator.anchor', defaultMessage: 'Date {name} at {at}', description: 'Indicator editor' },
  inAnchorDate: { id: 'app.studio.indicator.anchorDate', defaultMessage: 'Fixed date {name} at {at}', description: 'Indicator editor' },
  inAnchorName: { id: 'app.studio.indicator.anchorName', defaultMessage: '{anchor, select, periodStart {the period’s start} periodEnd {the period’s end} other {a fixed date}}', description: 'Indicator editor' },
  inFrom: { id: 'app.studio.indicator.from', defaultMessage: 'from', description: 'Indicator editor' },
  inTo: { id: 'app.studio.indicator.to', defaultMessage: 'to', description: 'Indicator editor' },
  inAnd: { id: 'app.studio.indicator.and', defaultMessage: 'and', description: 'Indicator editor' },
  inAmount: { id: 'app.studio.indicator.amount', defaultMessage: 'How many at {at}', description: 'Indicator editor' },
  inUnit: { id: 'app.studio.indicator.unit', defaultMessage: 'Unit at {at}', description: 'Indicator editor' },
  inDirection: { id: 'app.studio.indicator.direction', defaultMessage: 'Before or after at {at}', description: 'Indicator editor' },
  inDirectionName: { id: 'app.studio.indicator.directionName', defaultMessage: '{direction, select, before {before} other {after}}', description: 'Indicator editor' },
  inPeriods: { id: 'app.studio.indicator.periods', defaultMessage: 'How many periods at {at}', description: 'Indicator editor' },
  inPeriodsWord: { id: 'app.studio.indicator.periodsWord', defaultMessage: 'periods', description: 'Indicator editor' },
  inDisaggregations: { id: 'app.studio.indicator.disaggregations', defaultMessage: 'Disaggregations', description: 'Indicator editor' },
  inDisaggregationsHint: { id: 'app.studio.indicator.disaggregationsHint', defaultMessage: 'Each has a not recorded band, so the bands always add up to the total.', description: 'Indicator editor' },
  inNoDisaggregations: { id: 'app.studio.indicator.noDisaggregations', defaultMessage: 'Not disaggregated.', description: 'Indicator editor' },
  inByAge: { id: 'app.studio.indicator.byAge', defaultMessage: 'By age:', description: 'Indicator editor' },
  inByCode: { id: 'app.studio.indicator.byCode', defaultMessage: 'By a coded value:', description: 'Indicator editor' },
  inAgeSet: { id: 'app.studio.indicator.ageSet', defaultMessage: 'Age band set of disaggregation {at}', description: 'Indicator editor' },
  inCodedName: { id: 'app.studio.indicator.codedName', defaultMessage: 'Name of disaggregation {at}', description: 'Indicator editor' },
  inCodedCriteria: { id: 'app.studio.indicator.codedCriteria', defaultMessage: 'FHIRPath giving the code of disaggregation {at}', description: 'Indicator editor' },
  inRemoveDisaggregation: { id: 'app.studio.indicator.removeDisaggregation', defaultMessage: 'Remove disaggregation {at}', description: 'Indicator editor' },
  inAddAge: { id: 'app.studio.indicator.addAge', defaultMessage: '+ age bands', description: 'Indicator editor' },
  inAddCoded: { id: 'app.studio.indicator.addCoded', defaultMessage: '+ coded value', description: 'Indicator editor' },
  inSave: { id: 'app.studio.indicator.save', defaultMessage: 'Save', description: 'Indicator editor' },
  inSaved: { id: 'app.studio.indicator.saved', defaultMessage: 'Saved.', description: 'Indicator editor' },
  inStale: { id: 'app.studio.indicator.stale', defaultMessage: 'Someone saved this draft since you opened it, so nothing was saved. Reload to see their changes.', description: 'Indicator editor' },
  inReload: { id: 'app.studio.indicator.reload', defaultMessage: 'Reload', description: 'Indicator editor' },
  inFailed: { id: 'app.studio.indicator.failed', defaultMessage: 'It was not saved: {reason}', description: 'Indicator editor' },
  inTry: { id: 'app.studio.indicator.try', defaultMessage: 'Try', description: 'Indicator editor' },
  inTryHint: { id: 'app.studio.indicator.tryHint', defaultMessage: 'Central evaluates this definition, unsaved edits included, over the package’s test patients.', description: 'Indicator editor' },
  inPeriodType: { id: 'app.studio.indicator.periodType', defaultMessage: 'Period type', description: 'Indicator editor' },
  inPeriodTypeName: { id: 'app.studio.indicator.periodTypeName', defaultMessage: '{type, select, weekly {Weekly} monthly {Monthly} quarterly {Quarterly} semiAnnual {Six-monthly} annual {Yearly} financialJuly {Financial year from July} other {{type}}}', description: 'Indicator editor' },
  inPeriod: { id: 'app.studio.indicator.period', defaultMessage: 'Period', description: 'Indicator editor' },
  inPeriodExample: { id: 'app.studio.indicator.periodExample', defaultMessage: '{type, select, weekly {e.g. 2026W10} monthly {e.g. 2026-03} quarterly {e.g. 2026Q1} semiAnnual {e.g. 2026S1} annual {e.g. 2026} financialJuly {e.g. 2025July} other {}}', description: 'Indicator editor' },
  inTrying: { id: 'app.studio.indicator.trying', defaultMessage: 'Trying…', description: 'Indicator editor' },
  inTryFailed: { id: 'app.studio.indicator.tryFailed', defaultMessage: 'It could not be tried: {reason}', description: 'Indicator editor' },
  inPeriodRange: { id: 'app.studio.indicator.periodRange', defaultMessage: '{id}: {start} to {end}', description: 'Indicator editor' },
  inFigure: { id: 'app.studio.indicator.figure', defaultMessage: 'The figure', description: 'Indicator editor' },
  inValueShown: { id: 'app.studio.indicator.valueShown', defaultMessage: '{calculation, select, percentage {{value}%} rate {{value} per {per}} other {{value}}}', description: 'Indicator editor' },
  inParts: { id: 'app.studio.indicator.parts', defaultMessage: 'Numerator {count} of denominator {denominator}', description: 'Indicator editor' },
  inCountShown: { id: 'app.studio.indicator.countShown', defaultMessage: '{count} counted', description: 'Indicator editor' },
  inValuesShown: { id: 'app.studio.indicator.valuesShown', defaultMessage: '{count} values: sum {sum}, from {minimum} to {maximum}', description: 'Indicator editor' },
  inByTitle: { id: 'app.studio.indicator.byTitle', defaultMessage: 'By {title}', description: 'Indicator editor' },
  inBandColumn: { id: 'app.studio.indicator.bandColumn', defaultMessage: 'Band', description: 'Indicator editor' },
  inNumeratorColumn: { id: 'app.studio.indicator.numeratorColumn', defaultMessage: 'Numerator', description: 'Indicator editor' },
  inCountColumn: { id: 'app.studio.indicator.countColumn', defaultMessage: 'Count', description: 'Indicator editor' },
  inDenominatorColumn: { id: 'app.studio.indicator.denominatorColumn', defaultMessage: 'Denominator', description: 'Indicator editor' },
  inValueColumn: { id: 'app.studio.indicator.valueColumn', defaultMessage: 'Value', description: 'Indicator editor' },
  inErrors: { id: 'app.studio.indicator.errors', defaultMessage: 'Errors', description: 'Indicator editor' },
  inPatients: { id: 'app.studio.indicator.patients', defaultMessage: 'Test patients: {counted} of {total} counted', description: 'Indicator editor' },
  inPatientLine: { id: 'app.studio.indicator.patientLine', defaultMessage: '{name}: {counted, select, true {counted {count}} other {not counted}}{denominator, select, none {} other { · denominator {denominator}}}', description: 'Indicator editor' },
  inReason: { id: 'app.studio.indicator.reason', defaultMessage: '{reason, select, meets {counted} denominatorOnly {in the denominator only} doesNotMeet {does not meet the conditions} noEnrollment {belongs to no enrollment} outsidePeriod {outside the period} noPeriodDate {has no period date} error {could not be evaluated} other {{reason}}}', description: 'Indicator editor' },
  inRecordValue: { id: 'app.studio.indicator.recordValue', defaultMessage: 'value {value}{picked, select, true { (taken)} other {}}', description: 'Indicator editor' },
  inExplainSide: { id: 'app.studio.indicator.explainSide', defaultMessage: '{side, select, numerator {Numerator} denominator {Denominator} other {Conditions}}', description: 'Indicator editor: Try, the conditions of one side marked held or not on a record' },
  inHeld: { id: 'app.studio.indicator.held', defaultMessage: '{held, select, held {held:} notHeld {not held:} other {could not be evaluated:}}', description: 'Indicator editor: Try, whether a condition held on a record (read by screen readers)' },
  inMapping: { id: 'app.studio.indicator.mapping', defaultMessage: 'HMIS mapping', description: 'Indicator editor: where Reports lay the indicator out' },
  inMappingHint: { id: 'app.studio.indicator.mappingHint', defaultMessage: "Read-only: the rows of Reports that lay this indicator out. Mappings are edited in the Reporting package's Report designer.", description: 'Indicator editor' },
  inMappingNone: { id: 'app.studio.indicator.mappingNone', defaultMessage: 'No Report lays this indicator out yet.', description: 'Indicator editor' },
  inMappingRow: { id: 'app.studio.indicator.mappingRow', defaultMessage: '{title}, {section}: {label}', description: 'Indicator editor: a Report row laying the indicator out' },
  inMappingWhere: {
    id: 'app.studio.indicator.mappingWhere',
    defaultMessage: '{package}{published, select, true {} other { (draft)}} · version {pinned}{sent, select, true { · sent to DHIS2} other {}}',
    description: 'Indicator editor: the package version a Report row is in, the indicator version it pins, and whether it is sent to DHIS2',
  },
})

type ReportValues = {
  rdLoading: {}
  rdUnreadable: { reason: string }
  rdReadOnly: {}
  rdOpen: { path: string }
  rdOpenShort: {}
  rdOutline: {}
  rdSections: {}
  rdNoSections: {}
  rdUntitled: {}
  rdTableSummary: { rows: number; by: number; sent: string }
  rdListSummary: { rowPer: string; columns: number }
  rdProblemCount: { count: number }
  rdNewTable: {}
  rdAddTable: {}
  rdNewList: {}
  rdAddLineList: {}
  rdSave: {}
  rdSaved: {}
  rdStale: {}
  rdReload: {}
  rdFailed: { reason: string }
  rdPreview: {}
  rdPeriod: {}
  rdPeriodExample: { example: string }
  rdLevelShown: {}
  rdLevel: { level: string }
  rdPreviewFailed: { reason: string }
  rdComputing: {}
  rdPreviewHint: { patients: number; sources: string; start: string; end: string }
  rdListProgram: { program: string; episodeType: string; package: string; published: string }
  rdListPatient: {}
  rdListFailed: { reason: string }
  rdListEmpty: {}
  rdListCount: { rows: number; rowPer: string }
  rdListFacility: {}
  rdTitle: {}
  rdId: {id: string; path: string}
  rdPeriodType: {}
  rdPeriodTypeName: { type: string }
  rdLevels: {}
  rdSectionTitle: {}
  rdSectionId: {}
  rdMoveSectionUp: {}
  rdMoveSectionDown: {}
  rdRemoveSection: {}
  rdDisaggregations: {}
  rdNoDisaggregations: {}
  rdAxis: {}
  rdAxisOf: { name: string }
  rdAxisName: { axis: string }
  rdRemoveNamed: { name: string }
  rdSetToAdd: {}
  rdAddAgeBands: {}
  rdCode: {}
  rdExpression: {}
  rdAddCoded: {}
  rdOptions: {}
  rdOption: { option: string }
  rdIndicators: {}
  rdIndicatorToAdd: {}
  rdAddIndicator: {}
  rdRowLabel: { at: number }
  rdRowId: { at: number }
  rdRowIndicator: { at: number }
  rdUnknownIndicator: {}
  rdCalculation: { calculation: string }
  rdFacilityOnly: {}
  rdNotACount: {}
  rdDraftOnly: { package: string }
  rdNotBrokenDown: { name: string }
  rdMoveUpNamed: { name: string }
  rdMoveDownNamed: { name: string }
  rdRowPer: {}
  rdRowPerName: { rowPer: string }
  rdProgram: {}
  rdEnrollments: {}
  rdEnrollmentsName: { which: string }
  rdVisitTypes: {}
  rdAllVisits: {}
  rdVisitTypeSystem: { at: number }
  rdVisitTypeCode: { at: number }
  rdAddVisitType: {}
  rdCountedBy: {}
  rdCountedByNone: {}
  rdColumns: {}
  rdColumnLabel: { at: number }
  rdColumnId: { at: number }
  rdColumnAttribute: { at: number }
  rdColumnSystem: { at: number }
  rdColumnCode: { at: number }
  rdColumnPick: { at: number }
  rdPick: { pick: string }
  rdAttributeToAdd: {}
  rdAddAttribute: {}
  rdConceptLabel: {}
  rdConceptSystem: {}
  rdConceptCode: {}
  rdAddConcept: {}
  rdConceptSearch: {}
  rdConceptSystemToSearch: {}
  rdConceptSearchText: {}
  rdConceptSearching: {}
  rdConceptNone: {}
  rdConceptFound: { total: number; shown: number }
  rdConceptUse: { code: string; display: string }
  rdConceptSearchFailed: { reason: string }
  rdValidation: {}
  rdChecking: {}
  rdNoProblems: {}
  rdAuthority: {}
  rdNotShown: { reason: string; name: string }
  rdNa: {}
  rdValueShown: { value: string; calculation: string }
  rdIndicatorColumn: {}
  rdTotal: {}
  rdValue: {}
  rdChangeColumn: {}
  rdCell: {}
  rdCellCount: { count: number }
  rdCellParts: { count: number; denominator: number }
  rdCellPrevious: { period: string; value: string }
  rdCellNobody: {}
  rdCellCounted: { names: string }
  rdCellMapping: {}
  rdDhis2: {}
  rdDhis2Hint: {}
  rdDhis2None: {}
  rdDhis2Section: {}
  rdDhis2Row: {}
  rdDhis2Bands: {}
  rdDhis2Element: {}
  rdDhis2Combo: {}
  rdSentIfMapped: {}
  rdNotMapped: {}
}

export const reportMessages = defineMessages<ReportValues>({
  rdLoading: { id: 'app.studio.report.loading', defaultMessage: 'Loading the Report…', description: 'Report designer' },
  rdUnreadable: { id: 'app.studio.report.unreadable', defaultMessage: 'This file is not a Report the designer can edit ({reason}). Open it in the file editor instead.', description: 'Report designer' },
  rdReadOnly: { id: 'app.studio.report.readOnly', defaultMessage: 'Read only. Press Edit to change it in your draft.', description: 'Report designer' },
  rdOpen: { id: 'app.studio.report.open', defaultMessage: 'Open in the Report designer: {path}', description: 'Report designer' },
  rdOpenShort: { id: 'app.studio.report.openShort', defaultMessage: 'Open in the Report designer', description: 'Draft page: open a Report' },
  rdOutline: { id: 'app.studio.report.outline', defaultMessage: 'Outline', description: 'Report designer' },
  rdSections: { id: 'app.studio.report.sections', defaultMessage: 'Sections', description: 'Report designer' },
  rdNoSections: { id: 'app.studio.report.noSections', defaultMessage: 'No sections yet: add a table section or a line list.', description: 'Report designer' },
  rdUntitled: { id: 'app.studio.report.untitled', defaultMessage: 'Untitled section', description: 'Report designer' },
  rdTableSummary: { id: 'app.studio.report.tableSummary', defaultMessage: 'Table · {rows, plural, =0 {no indicators} one {# indicator} other {# indicators}}{by, plural, =0 {} one { · by # disaggregation} other { · by # disaggregations}}{sent, select, true { · sent to DHIS2} other {}}', description: 'Report designer' },
  rdListSummary: { id: 'app.studio.report.listSummary', defaultMessage: 'Line list · one row per {rowPer, select, visit {visit} other {enrollment}} · {columns, plural, =0 {no columns} one {# column} other {# columns}}', description: 'Report designer' },
  rdProblemCount: { id: 'app.studio.report.problemCount', defaultMessage: '{count, plural, one {# problem} other {# problems}}', description: 'Report designer' },
  rdNewTable: { id: 'app.studio.report.newTable', defaultMessage: 'New table section', description: 'Report designer' },
  rdAddTable: { id: 'app.studio.report.addTable', defaultMessage: '+ table section', description: 'Report designer' },
  rdNewList: { id: 'app.studio.report.newList', defaultMessage: 'New line list', description: 'Report designer' },
  rdAddLineList: { id: 'app.studio.report.addLineList', defaultMessage: '+ line list', description: 'Report designer' },
  rdSave: { id: 'app.studio.report.save', defaultMessage: 'Save', description: 'Report designer' },
  rdSaved: { id: 'app.studio.report.saved', defaultMessage: 'Saved.', description: 'Report designer' },
  rdStale: { id: 'app.studio.report.stale', defaultMessage: 'Someone saved this draft since you opened it. Nothing was kept: reload to see their changes.', description: 'Report designer' },
  rdReload: { id: 'app.studio.report.reload', defaultMessage: 'Reload', description: 'Report designer' },
  rdFailed: { id: 'app.studio.report.failed', defaultMessage: 'Saving failed: {reason}', description: 'Report designer' },
  rdPreview: { id: 'app.studio.report.preview', defaultMessage: 'Preview', description: 'Report designer' },
  rdPeriod: { id: 'app.studio.report.period', defaultMessage: 'Period', description: 'Report designer' },
  rdPeriodExample: { id: 'app.studio.report.periodExample', defaultMessage: 'Written like {example}', description: 'Report designer' },
  rdLevelShown: { id: 'app.studio.report.levelShown', defaultMessage: 'Level', description: 'Report designer' },
  rdLevel: { id: 'app.studio.report.level', defaultMessage: '{level, select, facility {Facility} district {District} region {Region} national {National} other {{level}}}', description: 'Report designer' },
  rdPreviewFailed: { id: 'app.studio.report.previewFailed', defaultMessage: 'The preview failed: {reason}', description: 'Report designer' },
  rdComputing: { id: 'app.studio.report.computing', defaultMessage: 'Computing…', description: 'Report designer' },
  rdPreviewHint: { id: 'app.studio.report.previewHint', defaultMessage: '{start} to {end}, over {patients, plural, one {# test patient} other {# test patients}} of {sources}. Click a cell to open its section.', description: 'Report designer' },
  rdListProgram: { id: 'app.studio.report.listProgram', defaultMessage: 'Enrollments in {program} ({episodeType}), from {package}{published, select, true {} other { (an open draft)}}.', description: 'Report designer' },
  rdListPatient: { id: 'app.studio.report.listPatient', defaultMessage: 'Test patient', description: 'Report designer' },
  rdListFailed: { id: 'app.studio.report.listFailed', defaultMessage: 'Its rows cannot be read: {reason}', description: 'Report designer' },
  rdListEmpty: { id: 'app.studio.report.listEmpty', defaultMessage: 'No test patient has a row in this period.', description: 'Report designer' },
  rdListCount: { id: 'app.studio.report.listCount', defaultMessage: '{rows, plural, one {# row} other {# rows}}, one per {rowPer, select, visit {visit} other {enrollment}}, over the test patients. On the facility’s edge it runs over its own patients.', description: 'Report designer' },
  rdListFacility: { id: 'app.studio.report.listFacility', defaultMessage: 'Line lists run at facility level only: choose Facility to see it.', description: 'Report designer' },
  rdTitle: { id: 'app.studio.report.title', defaultMessage: 'Title', description: 'Report designer' },
  rdId: { id: 'app.studio.report.id', defaultMessage: 'Id {id}, kept at {path}', description: 'Report designer' },
  rdPeriodType: { id: 'app.studio.report.periodType', defaultMessage: 'Period type', description: 'Report designer' },
  rdPeriodTypeName: { id: 'app.studio.report.periodTypeName', defaultMessage: '{type, select, weekly {Weekly} monthly {Monthly} quarterly {Quarterly} semiAnnual {Semi-annual} annual {Annual} financialJuly {Financial year from July} other {{type}}}', description: 'Report designer' },
  rdLevels: { id: 'app.studio.report.levels', defaultMessage: 'Levels it runs for', description: 'Report designer' },
  rdSectionTitle: { id: 'app.studio.report.sectionTitle', defaultMessage: 'Section title', description: 'Report designer' },
  rdSectionId: { id: 'app.studio.report.sectionId', defaultMessage: 'Section id', description: 'Report designer' },
  rdMoveSectionUp: { id: 'app.studio.report.moveSectionUp', defaultMessage: 'Move section up', description: 'Report designer' },
  rdMoveSectionDown: { id: 'app.studio.report.moveSectionDown', defaultMessage: 'Move section down', description: 'Report designer' },
  rdRemoveSection: { id: 'app.studio.report.removeSection', defaultMessage: 'Remove section', description: 'Report designer' },
  rdDisaggregations: { id: 'app.studio.report.disaggregations', defaultMessage: 'Disaggregations', description: 'Report designer' },
  rdNoDisaggregations: { id: 'app.studio.report.noDisaggregations', defaultMessage: 'None: one figure per indicator.', description: 'Report designer' },
  rdAxis: { id: 'app.studio.report.axis', defaultMessage: 'on', description: 'Report designer' },
  rdAxisOf: { id: 'app.studio.report.axisOf', defaultMessage: 'Lay {name} out on', description: 'Report designer' },
  rdAxisName: { id: 'app.studio.report.axisName', defaultMessage: '{axis, select, rows {rows} other {columns}}', description: 'Report designer' },
  rdRemoveNamed: { id: 'app.studio.report.removeNamed', defaultMessage: 'Remove {name}', description: 'Report designer' },
  rdSetToAdd: { id: 'app.studio.report.setToAdd', defaultMessage: 'Age band set to add', description: 'Report designer' },
  rdAddAgeBands: { id: 'app.studio.report.addAgeBands', defaultMessage: '+ age bands', description: 'Report designer' },
  rdCode: { id: 'app.studio.report.code', defaultMessage: 'Coded value: its name', description: 'Report designer' },
  rdExpression: { id: 'app.studio.report.expression', defaultMessage: 'FHIRPath giving the code', description: 'Report designer' },
  rdAddCoded: { id: 'app.studio.report.addCoded', defaultMessage: '+ coded value', description: 'Report designer' },
  rdOptions: { id: 'app.studio.report.options', defaultMessage: 'Options', description: 'Report designer' },
  rdOption: { id: 'app.studio.report.option', defaultMessage: '{option, select, columnTotals {Column totals (a Total row under each indicator)} rowTotals {Row totals (a Total column)} change {% change from the previous period} sendToDhis2 {Send to DHIS2} other {{option}}}', description: 'Report designer' },
  rdIndicators: { id: 'app.studio.report.indicators', defaultMessage: 'Indicators', description: 'Report designer' },
  rdIndicatorToAdd: { id: 'app.studio.report.indicatorToAdd', defaultMessage: 'Indicator to add', description: 'Report designer' },
  rdAddIndicator: { id: 'app.studio.report.addIndicator', defaultMessage: '+ indicator', description: 'Report designer' },
  rdRowLabel: { id: 'app.studio.report.rowLabel', defaultMessage: 'Label of row {at}', description: 'Report designer' },
  rdRowId: { id: 'app.studio.report.rowId', defaultMessage: 'Id of row {at}', description: 'Report designer' },
  rdRowIndicator: { id: 'app.studio.report.rowIndicator', defaultMessage: 'Indicator of row {at}', description: 'Report designer' },
  rdUnknownIndicator: { id: 'app.studio.report.unknownIndicator', defaultMessage: 'not found among published indicators or drafts', description: 'Report designer' },
  rdCalculation: { id: 'app.studio.report.calculation', defaultMessage: '{calculation, select, count {count} percentage {percentage} rate {rate} ratio {ratio} sum {sum} average {average} median {median} minimum {minimum} maximum {maximum} distinctCount {distinct count} other {{calculation}}}', description: 'Report designer' },
  rdFacilityOnly: { id: 'app.studio.report.facilityOnly', defaultMessage: 'facility only: it cannot be rebuilt from facility summaries', description: 'Report designer' },
  rdNotACount: { id: 'app.studio.report.notACount', defaultMessage: 'not a count', description: 'Report designer' },
  rdDraftOnly: { id: 'app.studio.report.draftOnly', defaultMessage: 'only in the draft {package}: publish it before this Report', description: 'Report designer' },
  rdNotBrokenDown: { id: 'app.studio.report.notBrokenDown', defaultMessage: 'Not broken down by {name}: its cells show n/a. A note, not an error.', description: 'Report designer' },
  rdMoveUpNamed: { id: 'app.studio.report.moveUpNamed', defaultMessage: 'Move {name} up', description: 'Report designer' },
  rdMoveDownNamed: { id: 'app.studio.report.moveDownNamed', defaultMessage: 'Move {name} down', description: 'Report designer' },
  rdRowPer: { id: 'app.studio.report.rowPer', defaultMessage: 'One row per', description: 'Report designer' },
  rdRowPerName: { id: 'app.studio.report.rowPerName', defaultMessage: '{rowPer, select, visit {visit (of chosen types, in the period)} other {enrollment in the program}}', description: 'Report designer' },
  rdProgram: { id: 'app.studio.report.program', defaultMessage: 'Program (canonical URL)', description: 'Report designer' },
  rdEnrollments: { id: 'app.studio.report.enrollments', defaultMessage: 'Enrollments', description: 'Report designer' },
  rdEnrollmentsName: { id: 'app.studio.report.enrollmentsName', defaultMessage: '{which, select, started {started in the period} other {active during the period}}', description: 'Report designer' },
  rdVisitTypes: { id: 'app.studio.report.visitTypes', defaultMessage: 'Visit types', description: 'Report designer' },
  rdAllVisits: { id: 'app.studio.report.allVisits', defaultMessage: 'None: every visit in the program.', description: 'Report designer' },
  rdVisitTypeSystem: { id: 'app.studio.report.visitTypeSystem', defaultMessage: 'System of visit type {at}', description: 'Report designer' },
  rdVisitTypeCode: { id: 'app.studio.report.visitTypeCode', defaultMessage: 'Code of visit type {at}', description: 'Report designer' },
  rdAddVisitType: { id: 'app.studio.report.addVisitType', defaultMessage: '+ visit type', description: 'Report designer' },
  rdCountedBy: { id: 'app.studio.report.countedBy', defaultMessage: 'Only those counted by', description: 'Report designer' },
  rdCountedByNone: { id: 'app.studio.report.countedByNone', defaultMessage: 'No indicator: everyone in the program', description: 'Report designer' },
  rdColumns: { id: 'app.studio.report.columns', defaultMessage: 'Columns', description: 'Report designer' },
  rdColumnLabel: { id: 'app.studio.report.columnLabel', defaultMessage: 'Heading of column {at}', description: 'Report designer' },
  rdColumnId: { id: 'app.studio.report.columnId', defaultMessage: 'Id of column {at}', description: 'Report designer' },
  rdColumnAttribute: { id: 'app.studio.report.columnAttribute', defaultMessage: 'Attribute of column {at}', description: 'Report designer' },
  rdColumnSystem: { id: 'app.studio.report.columnSystem', defaultMessage: 'Concept system of column {at}', description: 'Report designer' },
  rdColumnCode: { id: 'app.studio.report.columnCode', defaultMessage: 'Concept code of column {at}', description: 'Report designer' },
  rdColumnPick: { id: 'app.studio.report.columnPick', defaultMessage: 'Values shown in column {at}', description: 'Report designer' },
  rdPick: { id: 'app.studio.report.pick', defaultMessage: '{pick, select, latest {the latest} first {the first} all {all, with dates} count {times recorded} other {{pick}}}', description: 'Report designer' },
  rdAttributeToAdd: { id: 'app.studio.report.attributeToAdd', defaultMessage: 'Attribute to add', description: 'Report designer' },
  rdAddAttribute: { id: 'app.studio.report.addAttribute', defaultMessage: '+ attribute', description: 'Report designer' },
  rdConceptLabel: { id: 'app.studio.report.conceptLabel', defaultMessage: 'Concept column heading', description: 'Report designer' },
  rdConceptSystem: { id: 'app.studio.report.conceptSystem', defaultMessage: 'Concept system', description: 'Report designer' },
  rdConceptCode: { id: 'app.studio.report.conceptCode', defaultMessage: 'Concept code', description: 'Report designer' },
  rdAddConcept: { id: 'app.studio.report.addConcept', defaultMessage: '+ concept', description: 'Report designer' },
  rdConceptSearch: { id: 'app.studio.report.conceptSearch', defaultMessage: 'Find a concept', description: 'Report designer: the terminology search for a line list concept column' },
  rdConceptSystemToSearch: { id: 'app.studio.report.conceptSystemToSearch', defaultMessage: 'Code system', description: 'Report designer' },
  rdConceptSearchText: { id: 'app.studio.report.conceptSearchText', defaultMessage: 'Search by code or name', description: 'Report designer' },
  rdConceptSearching: { id: 'app.studio.report.conceptSearching', defaultMessage: 'Searching…', description: 'Report designer' },
  rdConceptNone: { id: 'app.studio.report.conceptNone', defaultMessage: 'No concept matches.', description: 'Report designer' },
  rdConceptFound: { id: 'app.studio.report.conceptFound', defaultMessage: '{total, plural, one {# concept} other {# concepts}} found{shown, select, 0 {} other {, showing the first {shown}}}', description: 'Report designer' },
  rdConceptUse: { id: 'app.studio.report.conceptUse', defaultMessage: 'Use {code} {display}', description: 'Report designer: fills the concept column to add with a found concept' },
  rdConceptSearchFailed: { id: 'app.studio.report.conceptSearchFailed', defaultMessage: 'The terminology could not be searched: {reason}', description: 'Report designer' },
  rdValidation: { id: 'app.studio.report.validation', defaultMessage: 'Validation', description: 'Report designer' },
  rdChecking: { id: 'app.studio.report.checking', defaultMessage: 'Checking…', description: 'Report designer' },
  rdNoProblems: { id: 'app.studio.report.noProblems', defaultMessage: 'No problems found.', description: 'Report designer' },
  rdAuthority: { id: 'app.studio.report.authority', defaultMessage: 'As Validation would refuse it, checked on central as you edit. Check now on the draft page remains the authority.', description: 'Report designer' },
  rdNotShown: { id: 'app.studio.report.notShown', defaultMessage: '{reason, select, notBrokenDown {The indicator is not broken down by {name}.} crossed {This crossing of bands was not counted in the preview.} facilityOnly {A median or distinct count cannot be rebuilt from facility summaries, so it shows at facility level only.} noIndicator {No indicator is pinned here.} other {Not computed yet.}}', description: 'Report designer' },
  rdNa: { id: 'app.studio.report.na', defaultMessage: 'n/a', description: 'Report designer' },
  rdValueShown: { id: 'app.studio.report.valueShown', defaultMessage: '{calculation, select, percentage {{value}%} other {{value}}}', description: 'Report designer' },
  rdIndicatorColumn: { id: 'app.studio.report.indicatorColumn', defaultMessage: 'Indicator', description: 'Report designer' },
  rdTotal: { id: 'app.studio.report.total', defaultMessage: 'Total', description: 'Report designer' },
  rdValue: { id: 'app.studio.report.value', defaultMessage: 'Value', description: 'Report designer' },
  rdChangeColumn: { id: 'app.studio.report.changeColumn', defaultMessage: '% change', description: 'Report designer' },
  rdCell: { id: 'app.studio.report.cell', defaultMessage: 'Selected cell', description: 'Report designer' },
  rdCellCount: { id: 'app.studio.report.cellCount', defaultMessage: 'Counted: {count}', description: 'Report designer' },
  rdCellParts: { id: 'app.studio.report.cellParts', defaultMessage: 'Numerator {count} of denominator {denominator}', description: 'Report designer' },
  rdCellPrevious: { id: 'app.studio.report.cellPrevious', defaultMessage: 'Previous period ({period}): {value}', description: 'Report designer' },
  rdCellNobody: { id: 'app.studio.report.cellNobody', defaultMessage: 'Nobody counted.', description: 'Report designer' },
  rdCellCounted: { id: 'app.studio.report.cellCounted', defaultMessage: 'Who it counted: {names}', description: 'Report designer' },
  rdCellMapping: { id: 'app.studio.report.cellMapping', defaultMessage: 'DHIS2 mapping: none yet. Mappings are added with #560.', description: 'Report designer' },
  rdDhis2: { id: 'app.studio.report.dhis2', defaultMessage: 'DHIS2 mapping', description: 'Report designer' },
  rdDhis2Hint: { id: 'app.studio.report.dhis2Hint', defaultMessage: 'Every cell of the sections sent to DHIS2, each to be mapped to a data element and category option combo with effective dates (#560). Totals and not recorded bands are sent only if mapped.', description: 'Report designer' },
  rdDhis2None: { id: 'app.studio.report.dhis2None', defaultMessage: 'No section is sent to DHIS2.', description: 'Report designer' },
  rdDhis2Section: { id: 'app.studio.report.dhis2Section', defaultMessage: 'Section', description: 'Report designer' },
  rdDhis2Row: { id: 'app.studio.report.dhis2Row', defaultMessage: 'Indicator', description: 'Report designer' },
  rdDhis2Bands: { id: 'app.studio.report.dhis2Bands', defaultMessage: 'Bands', description: 'Report designer' },
  rdDhis2Element: { id: 'app.studio.report.dhis2Element', defaultMessage: 'Data element', description: 'Report designer' },
  rdDhis2Combo: { id: 'app.studio.report.dhis2Combo', defaultMessage: 'Category option combo', description: 'Report designer' },
  rdSentIfMapped: { id: 'app.studio.report.sentIfMapped', defaultMessage: '(sent only if mapped)', description: 'Report designer' },
  rdNotMapped: { id: 'app.studio.report.notMapped', defaultMessage: 'Not mapped yet', description: 'Report designer' },
})

// The Dashboard designer (#879; studio.md § Dashboards): a standard Dashboard in a Reporting
// package, its Visualizations built like the explorer's, previewed over test patients.
type DashboardDesignerValues = {
  bdLoading: {}
  bdUnreadable: { reason: string }
  bdReadOnly: {}
  bdOpen: { path: string }
  bdOpenShort: {}
  bdOutline: {}
  bdTitle: {}
  bdDescription: {}
  bdId: { id: string; path: string }
  bdItems: {}
  bdNoItems: {}
  bdUntitled: {}
  bdSummary: { type: string; indicators: number; periods: string }
  bdProblemCount: { count: number }
  bdAddItem: {}
  bdNewItem: {}
  bdItemTitle: {}
  bdItemId: {}
  bdMoveUp: { title: string }
  bdMoveDown: { title: string }
  bdRemove: { title: string }
  bdIndicators: {}
  bdNoChoices: {}
  bdPeriods: {}
  bdLast: {}
  bdLastCount: {}
  bdRange: {}
  bdFrom: {}
  bdTo: {}
  bdRangeHint: {}
  bdFollow: {}
  bdFollow_myPlace: {}
  bdFollow_eachAreaUnder: {}
  bdValidation: {}
  bdChecking: {}
  bdNoProblems: {}
  bdCheckFailed: { reason: string }
  bdDashboardProblem: {}
  bdSave: {}
  bdSaved: {}
  bdStale: {}
  bdReload: {}
  bdFailed: { reason: string }
  bdPreview: {}
  bdAsOf: {}
  bdAsOfHint: {}
  bdComputing: {}
  bdPreviewFailed: { reason: string }
  bdPreviewHint: { patients: number; sources: string }
  bdSample: {}
  bdPeriodsShown: { from: string; to: string }
  bdMapAsTable: {}
  bdItemFailed: { reason: string }
}

export const dashboardDesignerMessages = defineMessages<DashboardDesignerValues>({
  bdLoading: { id: 'app.studio.dashboard.loading', defaultMessage: 'Loading the Dashboard…', description: 'Dashboard designer' },
  bdUnreadable: { id: 'app.studio.dashboard.unreadable', defaultMessage: 'This file is not a Dashboard the designer can edit ({reason}). Open it in the file editor instead.', description: 'Dashboard designer' },
  bdReadOnly: { id: 'app.studio.dashboard.readOnly', defaultMessage: 'Read only. Press Edit to change it in your draft.', description: 'Dashboard designer' },
  bdOpen: { id: 'app.studio.dashboard.open', defaultMessage: 'Open in the Dashboard designer: {path}', description: 'Dashboard designer' },
  bdOpenShort: { id: 'app.studio.dashboard.openShort', defaultMessage: 'Open in the Dashboard designer', description: 'Dashboard designer' },
  bdOutline: { id: 'app.studio.dashboard.outline', defaultMessage: 'Outline', description: 'Dashboard designer' },
  bdTitle: { id: 'app.studio.dashboard.title', defaultMessage: 'Dashboard title', description: 'Dashboard designer' },
  bdDescription: { id: 'app.studio.dashboard.description', defaultMessage: 'What it is for', description: 'Dashboard designer' },
  bdId: { id: 'app.studio.dashboard.id', defaultMessage: 'Id {id}, saved at {path}', description: 'Dashboard designer' },
  bdItems: { id: 'app.studio.dashboard.items', defaultMessage: 'Visualizations', description: 'Dashboard designer' },
  bdNoItems: { id: 'app.studio.dashboard.noItems', defaultMessage: 'No Visualizations yet: add a chart.', description: 'Dashboard designer' },
  bdUntitled: { id: 'app.studio.dashboard.untitled', defaultMessage: 'Untitled Visualization', description: 'Dashboard designer' },
  bdSummary: { id: 'app.studio.dashboard.summary', defaultMessage: '{type, select, column {Column chart} line {Line chart} table {Table} single {Single number} map {Map} other {{type}}} · {indicators, plural, =0 {no indicators} one {# indicator} other {# indicators}} · {periods}', description: 'Dashboard designer' },
  bdProblemCount: { id: 'app.studio.dashboard.problemCount', defaultMessage: '{count, plural, one {# problem} other {# problems}}', description: 'Dashboard designer' },
  bdAddItem: { id: 'app.studio.dashboard.addItem', defaultMessage: '+ visualization', description: 'Dashboard designer' },
  bdNewItem: { id: 'app.studio.dashboard.newItem', defaultMessage: 'New chart', description: 'Dashboard designer' },
  bdItemTitle: { id: 'app.studio.dashboard.itemTitle', defaultMessage: 'Visualization title', description: 'Dashboard designer' },
  bdItemId: { id: 'app.studio.dashboard.itemId', defaultMessage: 'Visualization id', description: 'Dashboard designer' },
  bdMoveUp: { id: 'app.studio.dashboard.moveUp', defaultMessage: 'Move {title} up', description: 'Dashboard designer' },
  bdMoveDown: { id: 'app.studio.dashboard.moveDown', defaultMessage: 'Move {title} down', description: 'Dashboard designer' },
  bdRemove: { id: 'app.studio.dashboard.remove', defaultMessage: 'Remove {title}', description: 'Dashboard designer' },
  bdIndicators: { id: 'app.studio.dashboard.indicators', defaultMessage: 'Indicators', description: 'Dashboard designer' },
  bdNoChoices: { id: 'app.studio.dashboard.noChoices', defaultMessage: 'No indicators in the Studio yet.', description: 'Dashboard designer' },
  bdPeriods: { id: 'app.studio.dashboard.periods', defaultMessage: 'Periods', description: 'Dashboard designer' },
  bdLast: { id: 'app.studio.dashboard.last', defaultMessage: 'The last', description: 'Dashboard designer' },
  bdLastCount: { id: 'app.studio.dashboard.lastCount', defaultMessage: 'How many periods', description: 'Dashboard designer' },
  bdRange: { id: 'app.studio.dashboard.range', defaultMessage: 'A fixed range', description: 'Dashboard designer' },
  bdFrom: { id: 'app.studio.dashboard.from', defaultMessage: 'From', description: 'Dashboard designer' },
  bdTo: { id: 'app.studio.dashboard.to', defaultMessage: 'To', description: 'Dashboard designer' },
  bdRangeHint: { id: 'app.studio.dashboard.rangeHint', defaultMessage: 'A standard Dashboard usually shows the last few periods, so it stays current.', description: 'Dashboard designer' },
  bdFollow: { id: 'app.studio.dashboard.follow', defaultMessage: 'Places', description: 'Dashboard designer' },
  bdFollow_myPlace: { id: 'app.studio.dashboard.follow_myPlace', defaultMessage: 'The viewer’s own place', description: 'Dashboard designer' },
  bdFollow_eachAreaUnder: { id: 'app.studio.dashboard.follow_eachAreaUnder', defaultMessage: 'Each area under the viewer’s place', description: 'Dashboard designer' },
  bdValidation: { id: 'app.studio.dashboard.validation', defaultMessage: 'Validation', description: 'Dashboard designer' },
  bdChecking: { id: 'app.studio.dashboard.checking', defaultMessage: 'Checking…', description: 'Dashboard designer' },
  bdNoProblems: { id: 'app.studio.dashboard.noProblems', defaultMessage: 'No problems: publication would accept this Dashboard.', description: 'Dashboard designer' },
  bdCheckFailed: { id: 'app.studio.dashboard.checkFailed', defaultMessage: 'Could not check: {reason}', description: 'Dashboard designer' },
  bdDashboardProblem: { id: 'app.studio.dashboard.dashboardProblem', defaultMessage: 'Dashboard', description: 'Dashboard designer' },
  bdSave: { id: 'app.studio.dashboard.save', defaultMessage: 'Save', description: 'Dashboard designer' },
  bdSaved: { id: 'app.studio.dashboard.saved', defaultMessage: 'Saved.', description: 'Dashboard designer' },
  bdStale: { id: 'app.studio.dashboard.stale', defaultMessage: 'Someone saved this draft since you opened it. Nothing was kept: reload to see their changes.', description: 'Dashboard designer' },
  bdReload: { id: 'app.studio.dashboard.reload', defaultMessage: 'Reload', description: 'Dashboard designer' },
  bdFailed: { id: 'app.studio.dashboard.failed', defaultMessage: 'Saving failed: {reason}', description: 'Dashboard designer' },
  bdPreview: { id: 'app.studio.dashboard.preview', defaultMessage: 'Preview', description: 'Dashboard designer' },
  bdAsOf: { id: 'app.studio.dashboard.asOf', defaultMessage: 'As of', description: 'Dashboard designer' },
  bdAsOfHint: { id: 'app.studio.dashboard.asOfHint', defaultMessage: 'The day the last periods run up to. The sample data is dated, so pick a day near it.', description: 'Dashboard designer' },
  bdComputing: { id: 'app.studio.dashboard.computing', defaultMessage: 'Computing the preview…', description: 'Dashboard designer' },
  bdPreviewFailed: { id: 'app.studio.dashboard.previewFailed', defaultMessage: 'The preview failed: {reason}', description: 'Dashboard designer' },
  bdPreviewHint: { id: 'app.studio.dashboard.previewHint', defaultMessage: 'Computed over {patients, plural, one {# test patient} other {# test patients}} ({sources}), standing for one place, whatever each Visualization follows.', description: 'Dashboard designer' },
  bdSample: { id: 'app.studio.dashboard.sample', defaultMessage: 'Test patients', description: 'Dashboard designer' },
  bdPeriodsShown: { id: 'app.studio.dashboard.periodsShown', defaultMessage: '{from} to {to}', description: 'Dashboard designer' },
  bdMapAsTable: { id: 'app.studio.dashboard.mapAsTable', defaultMessage: 'A map is previewed as a table: the test patients have no place on the map.', description: 'Dashboard designer' },
  bdItemFailed: { id: 'app.studio.dashboard.itemFailed', defaultMessage: 'Not computed: {reason}', description: 'Dashboard designer' },
})

// The Population Estimates editor (#553; ADR 0071): a year's estimate types, and the HMIS lead's
// CSV checked against the facility registry, with each district's totals before saving.
type PopulationValues = {
  peLoading: {}
  peUnreadable: { reason: string }
  peReadOnly: {}
  peHeading: { year: number }
  peTypes: {}
  peType: {}
  peCode: {}
  peHow: {}
  peGiven: {}
  peDerived: { percent: string; from: string }
  peFigures: { count: number; places: number }
  peTemplate: {}
  peTemplateHint: {}
  peImport: {}
  peFile: {}
  peCsv: {}
  peCheck: {}
  peChecking: {}
  peRefused: { count: number }
  peCheckFailed: { reason: string }
  peDistricts: { count: number }
  peDistrict: {}
  peNoEstimate: {}
  peProblems: { count: number }
  peSave: {}
  peSaved: {}
  peStale: {}
  peReload: {}
  peFailed: { reason: string }
}

export const populationMessages = defineMessages<PopulationValues>({
  peLoading: { id: 'app.studio.population.loading', defaultMessage: 'Loading the population estimates…', description: 'Population estimates editor' },
  peUnreadable: { id: 'app.studio.population.unreadable', defaultMessage: 'This file is not population estimates the Studio can read ({reason}). Open it in the file editor instead.', description: 'Population estimates editor' },
  peReadOnly: { id: 'app.studio.population.readOnly', defaultMessage: 'Read only. Press Edit to import figures into your draft.', description: 'Population estimates editor' },
  peHeading: { id: 'app.studio.population.heading', defaultMessage: 'Population estimates {year}', description: 'Population estimates editor: the heading' },
  peTypes: { id: 'app.studio.population.types', defaultMessage: 'Estimate types', description: 'Population estimates editor: the types table' },
  peType: { id: 'app.studio.population.type', defaultMessage: 'Type', description: 'Population estimates editor: column' },
  peCode: { id: 'app.studio.population.code', defaultMessage: 'Code', description: 'Population estimates editor: column' },
  peHow: { id: 'app.studio.population.how', defaultMessage: 'How it is given', description: 'Population estimates editor: column' },
  peGiven: { id: 'app.studio.population.given', defaultMessage: 'Given per place, in the CSV', description: 'Population estimates editor: a given type' },
  peDerived: { id: 'app.studio.population.derived', defaultMessage: '{percent}% of {from}', description: 'Population estimates editor: a derived type' },
  peFigures: { id: 'app.studio.population.figures', defaultMessage: '{count, plural, =0 {No figures yet.} one {# figure} other {# figures}}{places, plural, =0 {} one { for # place} other { for # places}}', description: 'Population estimates editor: how many figures the draft holds' },
  peTemplate: { id: 'app.studio.population.template', defaultMessage: 'Download the CSV template', description: 'Population estimates editor: the template link' },
  peTemplateHint: { id: 'app.studio.population.templateHint', defaultMessage: 'One row per place, its code from the facility registry in the place column, and a column per given type. Leave a cell empty where a place has no figure. Figures are summed upward, never split down.', description: 'Population estimates editor: how to fill the template' },
  peImport: { id: 'app.studio.population.import', defaultMessage: 'Import figures', description: 'Population estimates editor: the import section' },
  peFile: { id: 'app.studio.population.file', defaultMessage: 'CSV file', description: 'Population estimates editor: the file picker' },
  peCsv: { id: 'app.studio.population.csv', defaultMessage: 'Or paste the CSV', description: 'Population estimates editor: the text area' },
  peCheck: { id: 'app.studio.population.check', defaultMessage: 'Check against the registry', description: 'Population estimates editor: the check button' },
  peChecking: { id: 'app.studio.population.checking', defaultMessage: 'Checking…', description: 'Population estimates editor' },
  peRefused: { id: 'app.studio.population.refused', defaultMessage: 'The file cannot be imported: {count, plural, one {# problem} other {# problems}}', description: 'Population estimates editor: the file was refused' },
  peCheckFailed: { id: 'app.studio.population.checkFailed', defaultMessage: 'Could not check: {reason}', description: 'Population estimates editor' },
  peDistricts: { id: 'app.studio.population.districts', defaultMessage: 'District totals ({count, plural, one {# district} other {# districts}})', description: 'Population estimates editor: the totals table' },
  peDistrict: { id: 'app.studio.population.district', defaultMessage: 'District', description: 'Population estimates editor: column' },
  peNoEstimate: { id: 'app.studio.population.noEstimate', defaultMessage: 'no estimate', description: 'Population estimates editor: a district without a figure' },
  peProblems: { id: 'app.studio.population.problems', defaultMessage: '{count, plural, one {# problem} other {# problems}} publication would refuse', description: 'Population estimates editor' },
  peSave: { id: 'app.studio.population.save', defaultMessage: 'Save these figures to the draft', description: 'Population estimates editor: save' },
  peSaved: { id: 'app.studio.population.saved', defaultMessage: 'Saved.', description: 'Population estimates editor' },
  peStale: { id: 'app.studio.population.stale', defaultMessage: 'Someone saved this draft since you opened it. Nothing was kept: reload to see their changes.', description: 'Population estimates editor' },
  peReload: { id: 'app.studio.population.reload', defaultMessage: 'Reload', description: 'Population estimates editor' },
  peFailed: { id: 'app.studio.population.failed', defaultMessage: 'Saving failed: {reason}', description: 'Population estimates editor' },
})
