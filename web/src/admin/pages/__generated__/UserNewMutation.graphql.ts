/**
 * @generated SignedSource<<bce9df0b2b18228899ba46af622a9c23>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type UserNewMutation$variables = {
  email: string;
  isSuper: boolean;
  locationGrants: ReadonlyArray<string>;
  readOnlyLocationGrants: ReadonlyArray<string>;
};
export type UserNewMutation$data = {
  readonly createUser: {
    readonly email: string;
    readonly id: string;
  };
};
export type UserNewMutation = {
  response: UserNewMutation$data;
  variables: UserNewMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "email"
  },
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "isSuper"
  },
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "locationGrants"
  },
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "readOnlyLocationGrants"
  }
],
v1 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "email",
        "variableName": "email"
      },
      {
        "kind": "Variable",
        "name": "isSuper",
        "variableName": "isSuper"
      },
      {
        "kind": "Variable",
        "name": "locationGrants",
        "variableName": "locationGrants"
      },
      {
        "kind": "Variable",
        "name": "readOnlyLocationGrants",
        "variableName": "readOnlyLocationGrants"
      }
    ],
    "concreteType": "User",
    "kind": "LinkedField",
    "name": "createUser",
    "plural": false,
    "selections": [
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "id",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "email",
        "storageKey": null
      }
    ],
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Fragment",
    "metadata": null,
    "name": "UserNewMutation",
    "selections": (v1/*:: as any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Operation",
    "name": "UserNewMutation",
    "selections": (v1/*:: as any*/)
  },
  "params": {
    "cacheID": "3571735fa491d8f212623762f007c493",
    "id": null,
    "metadata": {},
    "name": "UserNewMutation",
    "operationKind": "mutation",
    "text": "mutation UserNewMutation(\n  $email: String!\n  $isSuper: Boolean!\n  $locationGrants: [String!]!\n  $readOnlyLocationGrants: [String!]!\n) {\n  createUser(email: $email, isSuper: $isSuper, locationGrants: $locationGrants, readOnlyLocationGrants: $readOnlyLocationGrants) {\n    id\n    email\n  }\n}\n"
  }
};
})();

(node as any).hash = "9920a6e87b878e107761c71b4968ed4d";

export default node;
