/**
 * @generated SignedSource<<3eb50dda66fc2fc38f1869952b582f06>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type UserEditRevokeOauthGrantMutation$variables = {
  id: string;
};
export type UserEditRevokeOauthGrantMutation$data = {
  readonly revokeOauthGrant: boolean;
};
export type UserEditRevokeOauthGrantMutation = {
  response: UserEditRevokeOauthGrantMutation$data;
  variables: UserEditRevokeOauthGrantMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "id"
  }
],
v1 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "id",
        "variableName": "id"
      }
    ],
    "kind": "ScalarField",
    "name": "revokeOauthGrant",
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Fragment",
    "metadata": null,
    "name": "UserEditRevokeOauthGrantMutation",
    "selections": (v1/*:: as any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Operation",
    "name": "UserEditRevokeOauthGrantMutation",
    "selections": (v1/*:: as any*/)
  },
  "params": {
    "cacheID": "e04ddb63e285ae11294da457b68f4f90",
    "id": null,
    "metadata": {},
    "name": "UserEditRevokeOauthGrantMutation",
    "operationKind": "mutation",
    "text": "mutation UserEditRevokeOauthGrantMutation(\n  $id: ID!\n) {\n  revokeOauthGrant(id: $id)\n}\n"
  }
};
})();

(node as any).hash = "b81c65eb7dd2fb1d0cc555b1cd35f4e0";

export default node;
