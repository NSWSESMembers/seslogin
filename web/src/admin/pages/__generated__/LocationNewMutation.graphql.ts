/**
 * @generated SignedSource<<08b61164e23f667c82a14b8438c83498>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
import { FragmentRefs } from "relay-runtime";
export type LocationNewMutation$variables = {
  name: string;
  nitcEnabled?: number | null | undefined;
};
export type LocationNewMutation$data = {
  readonly createLocation: {
    readonly id: string;
    readonly " $fragmentSpreads": FragmentRefs<"LocationList_item">;
  };
};
export type LocationNewMutation = {
  response: LocationNewMutation$data;
  variables: LocationNewMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "name"
  },
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "nitcEnabled"
  }
],
v1 = [
  {
    "kind": "Variable",
    "name": "name",
    "variableName": "name"
  },
  {
    "kind": "Variable",
    "name": "nitcEnabled",
    "variableName": "nitcEnabled"
  }
],
v2 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "id",
  "storageKey": null
};
return {
  "fragment": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Fragment",
    "metadata": null,
    "name": "LocationNewMutation",
    "selections": [
      {
        "alias": null,
        "args": (v1/*: any*/),
        "concreteType": "Location",
        "kind": "LinkedField",
        "name": "createLocation",
        "plural": false,
        "selections": [
          (v2/*: any*/),
          {
            "args": null,
            "kind": "FragmentSpread",
            "name": "LocationList_item"
          }
        ],
        "storageKey": null
      }
    ],
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Operation",
    "name": "LocationNewMutation",
    "selections": [
      {
        "alias": null,
        "args": (v1/*: any*/),
        "concreteType": "Location",
        "kind": "LinkedField",
        "name": "createLocation",
        "plural": false,
        "selections": [
          (v2/*: any*/),
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "name",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "enabled",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "nitcEnabled",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "nitcCompleteOnExport",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "lastSuccessfulMemberSync",
            "storageKey": null
          }
        ],
        "storageKey": null
      }
    ]
  },
  "params": {
    "cacheID": "4afd1345dc0ca261bd618037887d0120",
    "id": null,
    "metadata": {},
    "name": "LocationNewMutation",
    "operationKind": "mutation",
    "text": "mutation LocationNewMutation(\n  $name: String!\n  $nitcEnabled: Int\n) {\n  createLocation(name: $name, nitcEnabled: $nitcEnabled) {\n    id\n    ...LocationList_item\n  }\n}\n\nfragment LocationList_item on Location {\n  id\n  name\n  enabled\n  nitcEnabled\n  nitcCompleteOnExport\n  lastSuccessfulMemberSync\n}\n"
  }
};
})();

(node as any).hash = "b428b111f63ffe4b6cfa078f39cd003f";

export default node;
