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
  rrHint: { id: 'app.registryReview.hint', defaultMessage: 'Use the arrow keys to move through the queue and Enter to open an item.', description: 'Registry review: keyboard hint' },
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
